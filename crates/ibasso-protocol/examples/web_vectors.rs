//! JSON vectors for browser/Rust parity tests. No hardware access or extra dependencies.
use ibasso_protocol::{Filter, Gain, Output, Query, Report, Setting, VOLUME_TABLE};

fn report(report: Report) -> String {
    format!("{:?}", report.0)
}
fn main() {
    let queries = [
        ("firmware", Query::Firmware),
        ("volumeLeft", Query::VolumeLeft),
        ("volumeRight", Query::VolumeRight),
        ("filterLeft", Query::FilterLeft),
        ("filterRight", Query::FilterRight),
        ("gainLeft", Query::GainLeft),
        ("gainRight", Query::GainRight),
        ("outputLeft", Query::OutputLeft),
        ("outputRight", Query::OutputRight),
        ("eco", Query::Eco),
    ];
    println!("{{\"volumeTable\":{:?},\"queries\":[", VOLUME_TABLE);
    for (index, (name, query)) in queries.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        let bytes = query.report();
        print!(
            "{{\"name\":\"{name}\",\"report\":{},\"preamble\":{}}}",
            report(bytes),
            bytes.read_preamble().map(report).unwrap_or("null".into())
        );
    }
    print!("],\"settings\":[");
    let mut cases = Vec::new();
    for level in 0..=100 {
        for balance in -10..=10 {
            cases.push((
                format!("{{\"kind\":\"volume\",\"level\":{level},\"balance\":{balance}}}"),
                Setting::Volume { level, balance },
            ));
        }
    }
    for filter in Filter::ALL {
        cases.push((
            format!("{{\"kind\":\"filter\",\"value\":\"{}\"}}", filter.name()),
            Setting::Filter(filter),
        ));
    }
    for (name, gain) in [
        ("low", Gain::Low),
        ("medium", Gain::Medium),
        ("high", Gain::High),
    ] {
        cases.push((
            format!("{{\"kind\":\"gain\",\"value\":\"{name}\"}}"),
            Setting::Gain(gain),
        ));
    }
    for (name, output) in [("normal", Output::Normal), ("turbo", Output::Turbo)] {
        cases.push((
            format!("{{\"kind\":\"output\",\"value\":\"{name}\"}}"),
            Setting::Output(output),
        ));
    }
    for value in [false, true] {
        cases.push((
            format!("{{\"kind\":\"eco\",\"value\":{value}}}"),
            Setting::Eco(value),
        ));
    }
    for (index, (json, setting)) in cases.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        print!("{{\"setting\":{json},\"plan\":[");
        for (index, step) in setting.plan().unwrap().iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "{{\"write\":{},\"readback\":{},\"expected\":{}}}",
                report(step.write),
                report(step.readback),
                step.expected
            );
        }
        print!("]}}");
    }
    println!("]}}");
}
