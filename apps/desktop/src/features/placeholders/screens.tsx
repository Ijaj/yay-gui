import { Placeholder } from "../../components/Placeholder";
import { t } from "../../i18n";

export const HomeScreen = () => (
  <Placeholder title={t("nav.home")} description={t("placeholder.home.desc")} />
);
export const UpdatesScreen = () => (
  <Placeholder title={t("nav.updates")} description={t("placeholder.updates.desc")} />
);
export const InstalledScreen = () => (
  <Placeholder title={t("nav.installed")} description={t("placeholder.installed.desc")} />
);
export const MaintenanceScreen = () => (
  <Placeholder title={t("nav.maintenance")} description={t("placeholder.maintenance.desc")} />
);
export const SettingsScreen = () => (
  <Placeholder title={t("nav.settings")} description={t("placeholder.settings.desc")} />
);
