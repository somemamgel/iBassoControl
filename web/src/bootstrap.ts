import "./style.css";
import { t } from "./i18n";
void import("./main").catch((error) => {
    document.body.textContent = t("loadError") + String(error);
});
