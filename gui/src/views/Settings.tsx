import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DEFAULT_ACCENT, applyBaseTheme, getThemeConfig, injectStyle, hexToHslTriplet } from '@/theme';
import { showToast } from '@/toast';
import { confirmDialog } from '@/components/ConfirmDialog';
import { t, getLanguage, setLanguage, type Language } from '@/i18n';
import { Card, CardHeader, CardTitle, CardContent, CardDescription, CardFooter } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Html } from '@/components/Html';

const PREVIEW_STYLE_ID = 'photoranker-theme-override';
const DANGER_COLOR: Record<'dark' | 'light', string> = {
  dark: '#e55a6a',
  light: '#c73b4c',
};
const SIMILARITY_WARN_DISTANCE = 90;

/** El override que persiste `write_theme_override` — un solo `:root { }`
 *  (no separado por tema, igual que antes de la migración a React: el
 *  usuario tiene un único acento activo a la vez, atado al tema base que
 *  tenía seleccionado al guardar). Escribe `--primary`, el token real que
 *  leen `tailwind.config.js`/los componentes de shadcn — no `--color-accent`,
 *  que ya no lee ningún componente tras la migración. `--ring` (el color del
 *  borde de foco) queda deliberadamente fijo, sin atarse al acento — igual
 *  que `--color-focus-border` en la versión anterior, para que el foco del
 *  teclado siga siendo reconocible sin importar qué acento elija el usuario. */
function accentCss(accent: string): string {
  return `:root {
  --primary: ${hexToHslTriplet(accent)};
}`;
}

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace('#', ''), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function colorDistance(a: string, b: string): number {
  const [r1, g1, b1] = hexToRgb(a);
  const [r2, g2, b2] = hexToRgb(b);
  return Math.sqrt((r1 - r2) ** 2 + (g1 - g2) ** 2 + (b1 - b2) ** 2);
}

function toHex(color: string, fallbackTheme: 'dark' | 'light'): string {
  if (/^#[0-9a-fA-F]{6}$/.test(color)) return color;
  const probe = document.createElement('div');
  probe.style.color = color;
  document.body.appendChild(probe);
  const rgb = getComputedStyle(probe).color;
  document.body.removeChild(probe);
  const match = rgb.match(/\d+/g);
  if (!match) return DEFAULT_ACCENT[fallbackTheme];
  const [r, g, b] = match.map(Number);
  return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

export function SettingsView() {
  const [theme, setTheme] = useState<'dark' | 'light'>('dark');
  const [accent, setAccent] = useState('#6952e0');
  const [language, setLanguageState] = useState<Language>(getLanguage());
  const [isGuiManaged, setIsGuiManaged] = useState(true);
  const [themePath, setThemePath] = useState('');

  useEffect(() => {
    getThemeConfig().then(async (config) => {
      const currentTheme = config.theme === 'light' ? 'light' : 'dark';
      setTheme(currentTheme);
      setThemePath(config.theme_path || '');

      const guiManaged = await invoke<boolean>('theme_path_is_gui_managed', {
        themePath: config.theme_path ?? '',
      }).catch(() => true);
      setIsGuiManaged(guiManaged);

      // --primary es un triplete HSL ("H S% L%", ver index.css/tailwind.config.js),
      // no un color CSS válido por sí solo — hay que envolverlo en hsl(...)
      // antes de pasarlo por el probe de toHex().
      const primaryTriplet = getComputedStyle(document.documentElement)
        .getPropertyValue('--primary')
        .trim();
      const currentAccent = primaryTriplet ? `hsl(${primaryTriplet})` : DEFAULT_ACCENT[currentTheme];
      setAccent(toHex(currentAccent, currentTheme));
    });
  }, []);

  const handleAccentChange = (val: string) => {
    setAccent(val);
    injectStyle(PREVIEW_STYLE_ID, accentCss(val));
  };

  const resetAccent = () => {
    const val = toHex(DEFAULT_ACCENT[theme], theme);
    setAccent(val);
    injectStyle(PREVIEW_STYLE_ID, accentCss(val));
  };

  const saveTheme = async () => {
    if (!isGuiManaged) {
      const confirmed = await confirmDialog({
        title: t('settings.appearance.replaceCustomTitle'),
        message: t('settings.appearance.replaceCustomMessage', { path: themePath }),
        confirmLabel: t('settings.appearance.replaceCustomConfirm'),
        danger: true,
      });
      if (!confirmed) return;
    }
    try {
      await invoke('write_theme_config', { theme });
      await invoke('write_theme_override', { css: accentCss(accent) });
      applyBaseTheme(theme);
      showToast(t('settings.appearance.saved'));
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e), true);
    }
  };

  const handleLanguageChange = async (lang: Language) => {
    try {
      await invoke('write_language_config', { language: lang });
      setLanguage(lang);
      setLanguageState(lang);
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e), true);
    }
  };

  const distance = colorDistance(accent, DANGER_COLOR[theme]);
  const showWarning = distance < SIMILARITY_WARN_DISTANCE;

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      <h1 className="text-3xl font-bold tracking-tight">{t('settings.title')}</h1>

      <Tabs defaultValue="appearance">
        <TabsList>
          <TabsTrigger value="appearance">{t('settings.tabs.appearance')}</TabsTrigger>
          <TabsTrigger value="language">{t('settings.tabs.language')}</TabsTrigger>
        </TabsList>

        <TabsContent value="appearance">
          <Card>
            <CardHeader>
              <CardTitle className="text-lg">{t('settings.appearance.title')}</CardTitle>
              <CardDescription>{t('settings.appearance.description')}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-2">
                <Label htmlFor="base-theme-switch">{t('settings.appearance.baseTheme')}</Label>
                <div className="flex items-center gap-3">
                  <span className="text-sm text-muted-foreground">{t('settings.appearance.light')}</span>
                  <Switch
                    id="base-theme-switch"
                    checked={theme === 'dark'}
                    onCheckedChange={(checked) => setTheme(checked ? 'dark' : 'light')}
                  />
                  <span className="text-sm text-muted-foreground">{t('settings.appearance.dark')}</span>
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="accent-color-input">{t('settings.appearance.accentColor')}</Label>
                <div className="flex gap-3">
                  <Input
                    id="accent-color-input"
                    type="color"
                    value={accent}
                    onChange={(e) => handleAccentChange(e.target.value)}
                    className="w-16 h-10 p-1 cursor-pointer"
                  />
                  <Button variant="outline" onClick={resetAccent}>
                    {t('settings.appearance.reset')}
                  </Button>
                </div>
                {showWarning && (
                  <p className="text-sm text-destructive mt-2 bg-destructive/10 p-2 rounded">
                    {t('settings.appearance.accentWarning')}
                  </p>
                )}
              </div>

              {!isGuiManaged && (
                <Html
                  className="text-sm text-muted-foreground p-3 bg-muted rounded block"
                  html={t('settings.appearance.customThemeNotice', { path: themePath })}
                />
              )}
            </CardContent>
            <CardFooter>
              <Button onClick={saveTheme}>{t('common.save')}</Button>
            </CardFooter>
          </Card>
        </TabsContent>

        <TabsContent value="language">
          <Card>
            <CardHeader>
              <CardTitle className="text-lg">{t('settings.language.title')}</CardTitle>
              <CardDescription>{t('settings.language.description')}</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-2">
                <Label htmlFor="gui-language-switch">{t('settings.language.label')}</Label>
                <div className="flex items-center gap-3">
                  <span className="text-sm text-muted-foreground">{t('settings.language.spanish')}</span>
                  <Switch
                    id="gui-language-switch"
                    checked={language === 'en'}
                    onCheckedChange={(checked) => handleLanguageChange(checked ? 'en' : 'es')}
                  />
                  <span className="text-sm text-muted-foreground">{t('settings.language.english')}</span>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
