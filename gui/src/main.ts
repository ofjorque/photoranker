import '@fontsource-variable/inter';
import './theme/base.css';
import { loadTheme } from './theme';
import { initLanguage, onLanguageChange, t } from './i18n';
import { currentRoute, navigate, onRouteChange, type Route } from './router';
import { getProject, onProjectChange } from './state';
import { renderHome } from './views/Home';
import { renderBursts } from './views/Bursts';
import { renderTournament } from './views/Tournament';
import { renderCluster } from './views/Cluster';
import { renderVariables } from './views/Variables';
import { renderExport } from './views/Export';
import { renderSettings } from './views/Settings';
import { icons } from './components/icons';

// Una vista puede devolver una función de limpieza (ej. remover listeners
// globales de teclado de RankingBoard) — el router la llama antes de montar
// la próxima vista, para no acumular listeners al navegar (ver bug: Backspace
// dejaba de funcionar en otras pantallas tras visitar Torneo/Ráfagas).
type ViewCleanup = void | (() => void);
const routes: Record<Route, (el: HTMLElement) => ViewCleanup | Promise<ViewCleanup>> = {
  home: renderHome,
  bursts: renderBursts,
  tournament: renderTournament,
  cluster: renderCluster,
  variables: renderVariables,
  export: renderExport,
  settings: renderSettings,
};

// Orden del flujo de trabajo real (ver feedback de uso): cargar la carpeta,
// resolver ráfagas, definir/tagear variables, clusterizar (que ya puede usar
// esas variables), recién ahí el torneo principal, y exportar al final.
// Ajustes queda aparte al final — no es parte del flujo de trabajo secuencial.
// Funciones (no constantes de módulo) para que los labels se re-lean con
// `t()` cada vez que cambia el idioma activo, en vez de quedar congelados en
// el idioma que estaba activo cuando se evaluó el módulo.
function navItems(): Array<{ route: Route; label: string; icon: string }> {
  return [
    { route: 'home', label: t('main.nav.home'), icon: icons.folder },
    { route: 'bursts', label: t('main.nav.bursts'), icon: icons.burst },
    { route: 'variables', label: t('main.nav.variables'), icon: icons.tag },
    { route: 'cluster', label: t('main.nav.cluster'), icon: icons.cluster },
    { route: 'tournament', label: t('main.nav.tournament'), icon: icons.trophy },
    { route: 'export', label: t('main.nav.export'), icon: icons.export },
  ];
}
function settingsNavItem(): { route: Route; label: string; icon: string } {
  return { route: 'settings', label: t('main.nav.settings'), icon: icons.settings };
}

const NAV_COLLAPSED_KEY = 'photoranker-nav-collapsed';

async function bootstrap() {
  await initLanguage();
  await loadTheme();

  const app = document.querySelector<HTMLDivElement>('#app')!;
  const navCollapsed = localStorage.getItem(NAV_COLLAPSED_KEY) === 'true';
  app.innerHTML = `
    <div class="app-shell">
      <nav class="app-nav${navCollapsed ? ' app-nav--collapsed' : ''}" id="app-nav">
        <div class="brand">Photo<span class="accent">Ranker</span></div>
        <div class="project-path" id="nav-project-path"></div>
        <div id="nav-links"></div>
        <div class="nav-spacer"></div>
        <div id="nav-settings-link"></div>
        <button class="nav-collapse-btn" id="nav-collapse-btn" title="${t('main.nav.toggleCollapse')}"></button>
      </nav>
      <main class="app-main" id="app-main"></main>
    </div>
  `;

  const navEl = app.querySelector<HTMLElement>('#app-nav')!;
  const navLinks = app.querySelector<HTMLElement>('#nav-links')!;
  const navSettingsLink = app.querySelector<HTMLElement>('#nav-settings-link')!;
  const navProjectPath = app.querySelector<HTMLElement>('#nav-project-path')!;
  const navCollapseBtn = app.querySelector<HTMLButtonElement>('#nav-collapse-btn')!;
  const main = app.querySelector<HTMLElement>('#app-main')!;

  function navButton(item: { route: Route; label: string; icon: string }, active: Route): HTMLButtonElement {
    const btn = document.createElement('button');
    btn.className = 'nav-link' + (item.route === active ? ' active' : '');
    btn.title = item.label;
    btn.innerHTML = `<span class="nav-icon">${item.icon}</span><span class="nav-label">${item.label}</span>`;
    btn.addEventListener('click', () => navigate(item.route));
    return btn;
  }

  function renderNav() {
    const active = currentRoute();
    navLinks.innerHTML = '';
    for (const item of navItems()) {
      navLinks.appendChild(navButton(item, active));
    }
    navSettingsLink.innerHTML = '';
    navSettingsLink.appendChild(navButton(settingsNavItem(), active));
    navCollapseBtn.innerHTML = navEl.classList.contains('app-nav--collapsed')
      ? icons.chevronRight
      : icons.chevronLeft;
    navCollapseBtn.title = t('main.nav.toggleCollapse');
    const project = getProject();
    navProjectPath.textContent = project ? project.folderPath : t('main.nav.noProject');
  }

  navCollapseBtn.addEventListener('click', () => {
    const collapsed = navEl.classList.toggle('app-nav--collapsed');
    localStorage.setItem(NAV_COLLAPSED_KEY, String(collapsed));
    renderNav();
  });

  let currentCleanup: (() => void) | void;

  async function renderCurrentRoute() {
    currentCleanup?.();
    currentCleanup = undefined;
    renderNav();
    const route = currentRoute();
    currentCleanup = (await routes[route](main)) ?? undefined;
  }

  onRouteChange(renderCurrentRoute);
  onProjectChange(renderNav);
  // Cambiar de idioma re-renderiza toda la vista actual (no solo el nav) —
  // el copy de cada pantalla también depende de `t()`.
  onLanguageChange(renderCurrentRoute);

  await renderCurrentRoute();
}

bootstrap();
