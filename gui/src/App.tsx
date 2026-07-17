import { useEffect, useState } from 'react';
import { TitleBar } from '@/components/TitleBar';
import { initLanguage, onLanguageChange, t } from '@/i18n';
import { loadTheme } from '@/theme';
import { currentRoute, navigate, onRouteChange, type Route } from '@/router';
import { getProject, onProjectChange, type ProjectState } from '@/state';

import { Folder, Zap, Tags, Box, Trophy, ArrowUpRight, Settings, ChevronLeft, ChevronRight } from 'lucide-react';
import { Toaster } from '@/components/ui/sonner';
import {
  SidebarProvider,
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarFooter,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  useSidebar,
} from '@/components/ui/sidebar';

import { HomeView } from '@/views/Home.tsx';
import { BurstsView } from '@/views/Bursts.tsx';
import { VariablesView } from '@/views/Variables.tsx';
import { ClusterView } from '@/views/Cluster.tsx';
import { TournamentView } from '@/views/Tournament.tsx';
import { ExportView } from '@/views/Export.tsx';
import { SettingsView } from '@/views/Settings.tsx';

const NAV_COLLAPSED_KEY = 'photoranker-nav-collapsed';

/** Botón de colapsar propio (mismo ícono chevron que ya tenía la app) en vez
 *  del `SidebarTrigger` por defecto de shadcn (que usa un ícono `PanelLeft`
 *  fijo) — vive adentro del `SidebarFooter` para conservar el layout previo
 *  (Ajustes arriba, colapsar abajo, ambos "pineados" fuera del menú principal). */
function SidebarCollapseToggle() {
  const { state, toggleSidebar } = useSidebar();
  return (
    <button
      onClick={toggleSidebar}
      title={t('main.nav.toggleCollapse')}
      className="flex items-center gap-3 px-3 py-2 rounded-md hover:bg-sidebar-accent transition-colors text-sidebar-foreground"
    >
      {state === 'collapsed' ? <ChevronRight className="w-5 h-5 shrink-0" /> : <ChevronLeft className="w-5 h-5 shrink-0" />}
    </button>
  );
}

const activeMenuButtonClass =
  'data-[active=true]:bg-primary data-[active=true]:text-primary-foreground data-[active=true]:hover:bg-primary/90 data-[active=true]:hover:text-primary-foreground';

export function App() {
  const [route, setRoute] = useState<Route>(currentRoute());
  const [project, setProject] = useState<ProjectState | null>(getProject());
  // shadcn's SidebarProvider habla en términos de "open" (expandido), al
  // revés de la clave que ya usábamos (`...collapsed`) — se traduce en
  // handleNavOpenChange para no tener que migrar la clave de localStorage.
  const [navOpen, setNavOpen] = useState(() => localStorage.getItem(NAV_COLLAPSED_KEY) !== 'true');
  // Forzado de re-render en cambio de idioma: `t()` lee un valor mutable a
  // nivel de módulo (ver i18n/index.ts), no un estado de React, así que
  // ningún componente se entera solo — hay que forzarlo explícitamente acá,
  // en la raíz, para que se propague a todo el árbol (nav + vista activa).
  const [, forceLanguageRerender] = useState(0);

  useEffect(() => {
    initLanguage();
    loadTheme();

    const unRoute = onRouteChange(() => setRoute(currentRoute()));
    const unProject = onProjectChange(() => setProject(getProject()));
    const unLang = onLanguageChange(() => forceLanguageRerender((n) => n + 1));

    return () => {
      unRoute();
      unProject();
      unLang();
    };
  }, []);

  // Apaga el menú contextual nativo del webview (Recargar/Inspeccionar,
  // etc.) en toda la app — no aporta nada en una app de escritorio
  // terminada. Los `ContextMenu` propios de shadcn (Torneo, Ráfagas,
  // Clusters, Exportar) siguen funcionando igual: ya llaman
  // `preventDefault()` sobre este mismo evento en su propio trigger antes
  // de que este listener global corra, así que no hay conflicto — esto solo
  // tapa los lugares del resto de la app que no tienen un menú propio.
  useEffect(() => {
    const handler = (e: MouseEvent) => e.preventDefault();
    document.addEventListener('contextmenu', handler);
    return () => document.removeEventListener('contextmenu', handler);
  }, []);

  const handleNavOpenChange = (open: boolean) => {
    setNavOpen(open);
    localStorage.setItem(NAV_COLLAPSED_KEY, String(!open));
  };

  const navItems = [
    { route: 'home' as Route, label: t('main.nav.home'), icon: Folder },
    { route: 'bursts' as Route, label: t('main.nav.bursts'), icon: Zap },
    { route: 'variables' as Route, label: t('main.nav.variables'), icon: Tags },
    { route: 'cluster' as Route, label: t('main.nav.cluster'), icon: Box },
    { route: 'tournament' as Route, label: t('main.nav.tournament'), icon: Trophy },
    { route: 'export' as Route, label: t('main.nav.export'), icon: ArrowUpRight },
  ];

  return (
    <div className="h-screen w-screen flex flex-col overflow-hidden bg-background text-foreground">
      <TitleBar />
      {/* `transform` (sin valores) crea un "containing block" para el
          `position:fixed` que usa <Sidebar> internamente en desktop — sin
          esto, el sidebar se posiciona contra la ventana completa y queda
          por detrás/encima de la TitleBar en vez de debajo. */}
      <div className="flex flex-1 overflow-hidden transform">
        <SidebarProvider open={navOpen} onOpenChange={handleNavOpenChange} className="min-h-0 h-full">
          <Sidebar collapsible="icon">
            <SidebarHeader className="h-14 flex items-center border-b px-3 justify-center group-data-[collapsible=icon]:px-0">
              <div className="text-sm font-semibold truncate w-full group-data-[collapsible=icon]:hidden">
                {project ? project.folderPath : t('main.nav.noProject')}
              </div>
            </SidebarHeader>

            <SidebarContent>
              <SidebarMenu className="px-2 py-2 gap-1">
                {navItems.map((item) => (
                  <SidebarMenuItem key={item.route}>
                    <SidebarMenuButton
                      isActive={route === item.route}
                      tooltip={item.label}
                      onClick={() => navigate(item.route)}
                      className={activeMenuButtonClass}
                    >
                      <item.icon className="w-5 h-5" />
                      <span>{item.label}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarContent>

            <SidebarFooter className="gap-1">
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton
                    isActive={route === 'settings'}
                    tooltip={t('main.nav.settings')}
                    onClick={() => navigate('settings')}
                    className={activeMenuButtonClass}
                  >
                    <Settings className="w-5 h-5" />
                    <span>{t('main.nav.settings')}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
              <SidebarCollapseToggle />
            </SidebarFooter>
          </Sidebar>

          <main className="flex-1 overflow-auto bg-muted/20">
            {route === 'home' && <HomeView />}
            {route === 'bursts' && <BurstsView />}
            {route === 'variables' && <VariablesView />}
            {route === 'cluster' && <ClusterView />}
            {route === 'tournament' && <TournamentView />}
            {route === 'export' && <ExportView />}
            {route === 'settings' && <SettingsView />}
          </main>
        </SidebarProvider>
      </div>
      <Toaster richColors />
    </div>
  );
}
