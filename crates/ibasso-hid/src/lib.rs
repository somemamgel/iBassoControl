//! Device discovery and synchronous, bounded request/response transactions.
use hidapi::{DeviceInfo, HidApi, HidDevice};
use ibasso_protocol::{PID, PRODUCT, Query, Report, Setting, VID, WriteStep, response_data};
use std::{
    error::Error,
    fmt,
    time::{Duration, Instant},
};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message.into()))
}

pub fn devices() -> Result<Vec<DeviceInfo>> {
    let api = HidApi::new()?;
    Ok(api
        .device_list()
        .filter(|d| d.vendor_id() == VID && d.product_id() == PID)
        .cloned()
        .collect())
}

fn supported(info: &DeviceInfo) -> bool {
    info.vendor_id() == VID
        && info.product_id() == PID
        && info
            .product_string()
            .is_some_and(|s| s.trim().eq_ignore_ascii_case(PRODUCT))
        && info.usage_page() == 0x0c
        && info.usage() == 1
}

/// Abstract transport allows timeout, acknowledgement and failure behavior to be tested offline.
pub trait Transport {
    fn send(&self, report: &[u8; 17]) -> Result<()>;
    fn read(&self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize>;
}
impl Transport for HidDevice {
    fn send(&self, report: &[u8; 17]) -> Result<()> {
        // SET_REPORT(Output) on endpoint zero, matching the Android control transfer.
        self.send_output_report(report)?;
        Ok(())
    }
    fn read(&self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize> {
        Ok(self.read_timeout(buffer, timeout_ms)?)
    }
}

pub struct Client<T: Transport> {
    transport: T,
    timeout: Duration,
    trace: bool,
    healthy: bool,
}

impl Client<HidDevice> {
    pub fn open(path: Option<&str>, timeout: Duration, trace: bool) -> Result<Self> {
        let api = HidApi::new()?;
        let matches: Vec<_> = api
            .device_list()
            .filter(|d| supported(d) && path.is_none_or(|path| d.path().to_string_lossy() == path))
            .collect();
        let info = match matches.as_slice() {
            [] => {
                return Err(failure(
                    "No matching DC04 Pro HID device. Check the USB connection and device permissions; run `ibasso-cli list`.",
                ));
            }
            [info] => info,
            _ => {
                return Err(failure(
                    "Multiple DC04 Pro devices found; select one with --device <path> from `list`.",
                ));
            }
        };
        Self::new(info.open_device(&api)?, timeout, trace)
    }
}

impl<T: Transport> Client<T> {
    pub fn new(transport: T, timeout: Duration, trace: bool) -> Result<Self> {
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(failure(
                "timeout must be greater than zero and at most 30 seconds",
            ));
        }
        Ok(Self {
            transport,
            timeout,
            trace,
            healthy: true,
        })
    }

    /// A failed transaction poisons the session: a late reply cannot satisfy a later request.
    fn exchange(&mut self, report: Report) -> Result<Vec<u8>> {
        if !self.healthy {
            return Err(failure(
                "Session failed previously; reopen the device before retrying.",
            ));
        }
        let result = self.exchange_inner(report);
        if result.is_err() {
            self.healthy = false;
        }
        result
    }

    fn exchange_inner(&self, report: Report) -> Result<Vec<u8>> {
        let mut buffer = [0; 128];
        // Discard queued startup reports or native volume events; keep the drain bounded.
        let mut drained = false;
        for _ in 0..64 {
            if self.transport.read(&mut buffer, 0)? == 0 {
                drained = true;
                break;
            }
        }
        if !drained {
            return Err(failure("HID input queue did not become idle"));
        }
        let mut output = [0; 17];
        output[1..].copy_from_slice(&report.0);
        if self.trace {
            eprintln!("TX {:02x?}", report.0);
        }
        self.transport.send(&output)?;
        let started = Instant::now();
        loop {
            let remaining = self.timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break;
            }
            let count = self
                .transport
                .read(&mut buffer, remaining.as_millis().max(1) as i32)?;
            if count == 0 {
                break;
            }
            let bytes = buffer
                .get(..count)
                .ok_or_else(|| failure("invalid HID input length"))?;
            if self.trace {
                eprintln!("RX {bytes:02x?}");
            }
            if let Some(data) = response_data(bytes, report.id())? {
                return Ok(data.to_vec());
            }
        }
        Err(failure(format!(
            "Timed out after {} ms waiting for command 0x{:02x}; device may have disconnected or another controller may be using it.",
            self.timeout.as_millis(),
            report.id()
        )))
    }

    fn read_byte(&mut self, report: Report) -> Result<u8> {
        if let Some(preamble) = report.read_preamble() {
            self.exchange(preamble)?;
        }
        let data = self.exchange(report)?;
        ibasso_protocol::response_value(report, &data).map_err(|error| {
            self.healthy = false;
            failure(error.to_string())
        })
    }

    pub fn query(&mut self, query: Query) -> Result<u8> {
        self.read_byte(query.report())
    }

    pub fn status(&mut self) -> Result<Status> {
        Ok(Status {
            firmware: self.query(Query::Firmware)?,
            volume: [
                self.query(Query::VolumeLeft)?,
                self.query(Query::VolumeRight)?,
            ],
            filter: [
                self.query(Query::FilterLeft)?,
                self.query(Query::FilterRight)?,
            ],
            gain: [self.query(Query::GainLeft)?, self.query(Query::GainRight)?],
            output: [
                self.query(Query::OutputLeft)?,
                self.query(Query::OutputRight)?,
            ],
            eco: self.query(Query::Eco)?,
        })
    }

    pub fn apply(&mut self, setting: Setting) -> Result<()> {
        let plan = setting.plan()?;
        for (index, step) in plan.iter().enumerate() {
            if let Err(error) = self.apply_step(step) {
                self.healthy = false;
                return Err(failure(format!(
                    "Setting failed at step {}/{}: {error}. Device settings may be partially changed; query status before retrying.",
                    index + 1,
                    plan.len()
                )));
            }
        }
        Ok(())
    }

    fn apply_step(&mut self, step: &WriteStep) -> Result<()> {
        self.exchange(step.write)?;
        let actual = self.read_byte(step.readback)?;
        if actual != step.expected {
            return Err(failure(format!(
                "readback mismatch after command 0x{:02x}: expected 0x{:02x}, got 0x{actual:02x}",
                step.write.id(),
                step.expected
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub firmware: u8,
    /// FPGA volume mirror bytes, not the operating system's mixer volume.
    pub volume: [u8; 2],
    pub filter: [u8; 2],
    pub gain: [u8; 2],
    pub output: [u8; 2],
    pub eco: u8,
}

fn volume_text(raw: u8) -> String {
    match ibasso_protocol::volume_level(raw) {
        Some(level) => format!("{level}/100 (raw 0x{raw:02x})"),
        None => format!("between table steps (raw 0x{raw:02x})"),
    }
}
fn named(raw: u8, names: &[(u8, &str)]) -> String {
    format!(
        "{} (0x{raw:02x})",
        names
            .iter()
            .find(|(value, _)| *value == raw)
            .map_or("unknown", |(_, name)| name)
    )
}
impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Device: {PRODUCT}")?;
        writeln!(f, "FPGA: 1.{:x}", self.firmware)?;
        writeln!(
            f,
            "Volume mirror: L={} R={}",
            volume_text(self.volume[0]),
            volume_text(self.volume[1])
        )?;
        writeln!(
            f,
            "Balance raw L-R: {} (not always recoverable as an App slider value)",
            i16::from(self.volume[0]) - i16::from(self.volume[1])
        )?;
        let filters = ibasso_protocol::Filter::ALL.map(|filter| (filter.raw(), filter.name()));
        writeln!(
            f,
            "Filter: L={} R={}",
            named(self.filter[0], &filters),
            named(self.filter[1], &filters)
        )?;
        let gains = [(0, "low"), (0x20, "medium"), (0x31, "high")];
        writeln!(
            f,
            "Gain: L={} R={}",
            named(self.gain[0], &gains),
            named(self.gain[1], &gains)
        )?;
        let outputs = [(0x1c, "normal"), (0x1e, "turbo")];
        writeln!(
            f,
            "Output: L={} R={}",
            named(self.output[0], &outputs),
            named(self.output[1], &outputs)
        )?;
        write!(f, "ECO: {}", named(self.eco, &[(0, "off"), (0x20, "on")]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque};

    struct Fake {
        script: RefCell<VecDeque<(Report, Vec<Vec<u8>>)>>,
        pending: RefCell<VecDeque<Vec<u8>>>,
        sent: RefCell<Vec<[u8; 17]>>,
    }
    impl Fake {
        fn new(script: Vec<(Report, Vec<Vec<u8>>)>) -> Self {
            Self {
                script: RefCell::new(script.into()),
                pending: RefCell::new(VecDeque::new()),
                sent: RefCell::new(Vec::new()),
            }
        }
    }
    impl Transport for Fake {
        fn send(&self, report: &[u8; 17]) -> Result<()> {
            let (expected, replies) = self
                .script
                .borrow_mut()
                .pop_front()
                .expect("unexpected transaction");
            assert_eq!(report[0], 0, "unnumbered HID report ID");
            assert_eq!(report[1..], expected.0);
            self.sent.borrow_mut().push(*report);
            self.pending.borrow_mut().extend(replies);
            Ok(())
        }
        fn read(&self, buffer: &mut [u8], _: i32) -> Result<usize> {
            if let Some(bytes) = self.pending.borrow_mut().pop_front() {
                buffer[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            } else {
                Ok(0)
            }
        }
    }
    fn reply(id: u8, data: &[u8]) -> Vec<u8> {
        let mut result = vec![0; 32];
        result[6] = id;
        result[7] = data.len() as u8;
        result[8..8 + data.len()].copy_from_slice(data);
        result
    }
    fn client(fake: Fake) -> Client<Fake> {
        Client::new(fake, Duration::from_millis(10), false).unwrap()
    }

    #[test]
    fn skips_unsolicited_reports_and_matches_response() {
        let mut notification = reply(0x58, &[0xff]);
        notification[4] = 0xfe;
        notification[5] = 1;
        let fake = Fake::new(vec![(
            Query::Firmware.report(),
            vec![
                vec![0; 32],
                notification,
                reply(0x41, &[90]),
                reply(0x58, &[0x58]),
            ],
        )]);
        assert_eq!(client(fake).query(Query::Firmware).unwrap(), 0x58);
    }

    #[test]
    fn timeout_prevents_late_response_reuse() {
        let mut client = client(Fake::new(vec![(
            Query::Firmware.report(),
            vec![reply(0x41, &[90])],
        )]));
        assert!(
            client
                .query(Query::Firmware)
                .unwrap_err()
                .to_string()
                .contains("Timed out")
        );
        assert!(
            client
                .query(Query::Firmware)
                .unwrap_err()
                .to_string()
                .contains("reopen")
        );
        assert_eq!(client.transport.sent.borrow().len(), 1);
    }

    #[test]
    fn selects_map_before_extended_reads() {
        let read = Query::FilterLeft.report();
        let preamble = read.read_preamble().unwrap();
        assert_eq!(
            preamble.0,
            [0x52, 0x11, 0x88, 0x60, 0, 0, 4, 9, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        let fake = Fake::new(vec![
            (preamble, vec![reply(0x52, &[])]),
            (read, vec![reply(0x4b, &[0x42; 4])]),
        ]);
        assert_eq!(client(fake).query(Query::FilterLeft).unwrap(), 0x42);
    }

    #[test]
    fn validates_payload_size_instead_of_reading_padding() {
        let fake = Fake::new(vec![(Query::Firmware.report(), vec![reply(0x58, &[])])]);
        assert!(
            client(fake)
                .query(Query::Firmware)
                .unwrap_err()
                .to_string()
                .contains("expected 1 bytes")
        );
    }

    #[test]
    fn verifies_every_register_in_volume_plan() {
        let setting = Setting::Volume {
            level: 30,
            balance: 0,
        };
        let mut script = Vec::new();
        for step in setting.plan().unwrap() {
            script.push((step.write, vec![reply(step.write.id(), &[])]));
            let count = if let Some(preamble) = step.readback.read_preamble() {
                script.push((preamble, vec![reply(preamble.id(), &[])]));
                4
            } else {
                1
            };
            script.push((
                step.readback,
                vec![reply(step.readback.id(), &vec![step.expected; count])],
            ));
        }
        let mut client = client(Fake::new(script));
        client.apply(setting).unwrap();
        assert!(client.transport.script.borrow().is_empty());
        assert_eq!(client.transport.sent.borrow().len(), 28);
    }

    #[test]
    fn stops_on_readback_mismatch_and_reports_partial_change() {
        let setting = Setting::Filter(ibasso_protocol::Filter::D1);
        let step = setting.plan().unwrap()[0];
        let preamble = step.readback.read_preamble().unwrap();
        let fake = Fake::new(vec![
            (step.write, vec![reply(step.write.id(), &[])]),
            (preamble, vec![reply(preamble.id(), &[])]),
            (step.readback, vec![reply(step.readback.id(), &[0x22; 4])]),
        ]);
        let mut client = client(fake);
        let error = client.apply(setting).unwrap_err().to_string();
        assert!(error.contains("readback mismatch"));
        assert!(error.contains("partially changed"));
        assert_eq!(client.transport.sent.borrow().len(), 3);
    }

    #[test]
    fn rejects_invalid_setting_without_transport_calls() {
        let mut client = client(Fake::new(vec![]));
        assert!(
            client
                .apply(Setting::Volume {
                    level: 101,
                    balance: 0
                })
                .is_err()
        );
        assert!(client.transport.sent.borrow().is_empty());
    }
}
