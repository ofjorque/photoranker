// Barra de menú clásica (Archivo/Ver/Ayuda) oculta por defecto — aparece al
// presionar F10 y se vuelve a esconder al elegir una opción, hacer clic
// afuera, o presionar Escape/F10 de nuevo. Misma convención clásica de
// Windows que "Alt o F10 abre el menú" (Explorador de archivos, etc.) — se
// usa F10 en vez de Alt porque Alt es una "tecla de sistema" (WM_SYSKEYDOWN)
// que WebView2 suele absorber para su propio manejo de aceleradores antes de
// entregarla como keydown/keyup normal a JS (ver tauri-apps/tauri#13919);
// F10 no tiene ese problema. Construida a mano con el `Menubar` de shadcn en
// vez del menú nativo de Tauri, para que se vea consistente con el resto de
// la UI (la ventana no tiene decoraciones nativas, ver TitleBar).
import { useEffect, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarTrigger,
} from '@/components/ui/menubar';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { navigate, type Route } from '@/router';
import { t } from '@/i18n';

const REPO_URL = 'https://github.com/ofjorque/photoranker';

const VIEW_ITEMS: Array<{ route: Route; labelKey: string }> = [
  { route: 'home', labelKey: 'main.nav.home' },
  { route: 'bursts', labelKey: 'main.nav.bursts' },
  { route: 'variables', labelKey: 'main.nav.variables' },
  { route: 'cluster', labelKey: 'main.nav.cluster' },
  { route: 'tournament', labelKey: 'main.nav.tournament' },
  { route: 'export', labelKey: 'main.nav.export' },
];

export function AppMenu() {
  const [visible, setVisible] = useState(false);
  // Menú desplegado en este momento ('' = ninguno, solo la fila de triggers
  // visible) — controlado a mano para poder engancharnos a cuándo Radix lo
  // cierra (selección, Escape, clic afuera del contenido "portaled") y
  // esconder toda la barra en ese momento, ver `handleMenuValueChange`.
  const [openMenu, setOpenMenu] = useState('');
  const [aboutOpen, setAboutOpen] = useState(false);
  const [version, setVersion] = useState('');
  const barRef = useRef<HTMLDivElement>(null);
  // true una vez que se abrió algún desplegable desde que se mostró la
  // barra — evita confundir "recién apareció, nada abierto todavía" con
  // "se acaba de cerrar un desplegable que sí estaba abierto".
  const openedRef = useRef(false);

  useEffect(() => {
    getVersion().then(setVersion);
  }, []);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'F10' && !e.repeat) {
        e.preventDefault();
        setVisible((prev) => !prev);
        return;
      }
      if (e.key === 'Escape' && visible && !openMenu) {
        setVisible(false);
      }
    }
    function onPointerDown(e: PointerEvent) {
      // Solo actúa mientras no hay ningún desplegable abierto: su contenido
      // se renderiza en un portal fuera de `barRef`, así que un clic ahí
      // parecería "afuera" y cerraría todo antes de que el ítem reciba el
      // clic. Con un desplegable abierto, Radix ya maneja el cierre por su
      // cuenta (clic afuera, Escape) vía `onValueChange`, ver más abajo.
      if (visible && !openMenu && barRef.current && !barRef.current.contains(e.target as Node)) {
        setVisible(false);
      }
    }
    document.addEventListener('keydown', onKeyDown);
    document.addEventListener('pointerdown', onPointerDown);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('pointerdown', onPointerDown);
    };
  }, [visible, openMenu]);

  useEffect(() => {
    if (visible) {
      barRef.current?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus();
    }
  }, [visible]);

  function handleMenuValueChange(value: string) {
    setOpenMenu(value);
    if (value) {
      openedRef.current = true;
    } else if (openedRef.current) {
      // Un desplegable que estaba abierto se acaba de cerrar (selección,
      // Escape, o clic afuera) — esconder toda la barra, no solo ese menú.
      openedRef.current = false;
      setVisible(false);
    }
  }

  return (
    <>
      {visible && (
        <div
          ref={barRef}
          className="absolute top-8 left-0 z-[2000] border-b bg-background shadow-md"
        >
          <Menubar
            value={openMenu}
            onValueChange={handleMenuValueChange}
            className="rounded-none border-0 border-b-0 shadow-none"
          >
            <MenubarMenu value="file">
              <MenubarTrigger>{t('appMenu.file')}</MenubarTrigger>
              <MenubarContent>
                <MenubarItem onSelect={() => navigate('home')}>
                  {t('appMenu.file.newProject')}
                </MenubarItem>
                <MenubarItem onSelect={() => navigate('settings')}>
                  {t('appMenu.file.settings')}
                </MenubarItem>
                <MenubarSeparator />
                <MenubarItem onSelect={() => getCurrentWindow().close()}>
                  {t('appMenu.file.exit')}
                </MenubarItem>
              </MenubarContent>
            </MenubarMenu>

            <MenubarMenu value="view">
              <MenubarTrigger>{t('appMenu.view')}</MenubarTrigger>
              <MenubarContent>
                {VIEW_ITEMS.map((item) => (
                  <MenubarItem key={item.route} onSelect={() => navigate(item.route)}>
                    {t(item.labelKey)}
                  </MenubarItem>
                ))}
              </MenubarContent>
            </MenubarMenu>

            <MenubarMenu value="help">
              <MenubarTrigger>{t('appMenu.help')}</MenubarTrigger>
              <MenubarContent>
                <MenubarItem onSelect={() => setAboutOpen(true)}>
                  {t('appMenu.help.about')}
                </MenubarItem>
                <MenubarItem onSelect={() => openUrl(REPO_URL)}>
                  {t('appMenu.help.repo')}
                </MenubarItem>
                <MenubarItem onSelect={() => openUrl(`${REPO_URL}/issues`)}>
                  {t('appMenu.help.reportIssue')}
                </MenubarItem>
              </MenubarContent>
            </MenubarMenu>
          </Menubar>
        </div>
      )}

      {/* Fuera del `{visible && ...}` a propósito: si viviera adentro, elegir
          "Acerca de" lo desmontaría en el mismo instante en que Radix cierra
          el desplegable (dispara `handleMenuValueChange('')` -> esconde la
          barra), antes de que el diálogo llegue a mostrarse. */}
      <Dialog open={aboutOpen} onOpenChange={setAboutOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>
              Photo<span className="text-primary">Ranker</span>
            </DialogTitle>
            <DialogDescription>{t('appMenu.about.description')}</DialogDescription>
          </DialogHeader>
          <div className="text-sm text-muted-foreground space-y-1">
            <div>{t('appMenu.about.version', { version })}</div>
            <div>{t('appMenu.about.license')}</div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
