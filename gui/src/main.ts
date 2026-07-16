import '@fontsource-variable/inter';
import './theme/base.css';
import { loadTheme } from './theme';
import { currentRoute, navigate, onRouteChange, type Route } from './router';
import { getProject, onProjectChange } from './state';
import { renderHome } from './views/Home';
import { renderBursts } from './views/Bursts';
import { renderTournament } from './views/Tournament';
import { renderCluster } from './views/Cluster';
import { renderVariables } from './views/Variables';
import { renderExport } from './views/Export';

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
};

const navItems: Array<{ route: Route; label: string; icon: string }> = [
  { route: 'home', label: 'Proyecto', icon: '🗂️' },
  { route: 'bursts', label: 'Ráfagas', icon: '⚡' },
  { route: 'tournament', label: 'Torneo', icon: '🏆' },
  { route: 'cluster', label: 'Clustering', icon: '🧩' },
  { route: 'variables', label: 'Variables', icon: '🏷️' },
  { route: 'export', label: 'Ranking & Export', icon: '📤' },
];

async function bootstrap() {
  await loadTheme();

  const app = document.querySelector<HTMLDivElement>('#app')!;
  app.innerHTML = `
    <div class="app-shell">
      <nav class="app-nav">
        <div class="brand">Photo<span class="accent">Ranker</span></div>
        <div class="project-path" id="nav-project-path"></div>
        <div id="nav-links"></div>
      </nav>
      <main class="app-main" id="app-main"></main>
    </div>
  `;

  const navLinks = app.querySelector<HTMLElement>('#nav-links')!;
  const navProjectPath = app.querySelector<HTMLElement>('#nav-project-path')!;
  const main = app.querySelector<HTMLElement>('#app-main')!;

  function renderNav() {
    const active = currentRoute();
    navLinks.innerHTML = '';
    for (const item of navItems) {
      const btn = document.createElement('button');
      btn.className = 'nav-link' + (item.route === active ? ' active' : '');
      btn.innerHTML = `<span>${item.icon}</span><span>${item.label}</span>`;
      btn.addEventListener('click', () => navigate(item.route));
      navLinks.appendChild(btn);
    }
    const project = getProject();
    navProjectPath.textContent = project ? project.folderPath : 'Ningún proyecto abierto';
  }

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

  await renderCurrentRoute();
}

bootstrap();
