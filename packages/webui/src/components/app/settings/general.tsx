import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@repo/ui-solid/base/select";
import { CardX } from "@repo/ui-solid/custom/card";
import { getLocale, setLocale, locales } from "@repo/shared/i18n/paraglide/runtime";
import { m } from "@repo/shared/i18n/paraglide/messages";
import {
  tColorScheme,
  tLocaleName,
  type ColorSchemeKey,
  type LocaleNameKey,
} from "@repo/shared/i18n/utils";
import { useTheme } from "@repo/ui-solid/theme";
const languages = [
  { value: "en", label: tLocaleName("en") },
  { value: "zh-cn", label: tLocaleName("zh-cn") },
];
type LanguageOption = (typeof languages)[number];
export const GeneralSettings = () => {
  return (
    <div>
      <h2>{m.general()}</h2>
      <CardX
        title={m.language()}
        description={m.settings_language_description()}
        size="sm"
        Footer={
          <Select
            value={{
              value: getLocale(),
              label: tLocaleName(getLocale()),
            }}
            optionValue="value"
            optionTextValue="label"
            onChange={(v) => setLocale(v?.value ?? "en")}
            options={languages}

            itemComponent={(props) => (
              <SelectItem item={props.item}>{props.item.rawValue.label}</SelectItem>
            )}
          >
            <SelectTrigger class="w-45">
              <SelectValue<LanguageOption>>{(state) => state.selectedOption().label}</SelectValue>
            </SelectTrigger>
            <SelectContent />
          </Select>
        }
      />
      <AppearanceSettings />
    </div>
  );
};

const AppearanceSettings = () => {
  const { theme, setTheme } = useTheme();
  const colorScheme = () => (theme() === "auto" ? "system" : (theme() as ColorSchemeKey));
  const setColorScheme = (scheme: ColorSchemeKey) => {
    if (scheme === "system") {
      setTheme("auto");
    } else {
      setTheme(scheme);
    }
  };
  // 系统, 浅色, 深色
  const colorSchemeOptions = [
    { value: "system", label: m.system() },
    { value: "light", label: m.light() },
    { value: "dark", label: m.dark() },
  ];
  type ColorSchemeOption = (typeof colorSchemeOptions)[number];
  return (
    <>
      <h3>{m.appearance()}</h3>
      <CardX
        title={m.color_scheme()}
        description={m.settings_color_scheme()}
        size="sm"
        Footer={
          <Select
            value={{
              value: colorScheme(),
              label: tColorScheme(colorScheme()),
            }}
            optionValue="value"
            optionTextValue="label"
            onChange={(v) => setColorScheme(v?.value ?? "system")}
            options={colorSchemeOptions}
            itemComponent={(props) => (
              <SelectItem item={props.item}>{props.item.rawValue.label}</SelectItem>
            )}
          >
            <SelectTrigger class="w-45">
              <SelectValue<ColorSchemeOption>>
                {(state) => state.selectedOption().label}
              </SelectValue>
            </SelectTrigger>
            <SelectContent />
          </Select>
        }
      />
    </>
  );
};
