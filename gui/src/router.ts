// Router mínimo por hash, sin dependencias externas.
export type Route = 'home' | 'bursts' | 'tournament' | 'cluster' | 'variables' | 'export';

const validRoutes: Route[] = ['home', 'bursts', 'tournament', 'cluster', 'variables', 'export'];

export function currentRoute(): Route {
  const hash = window.location.hash.replace('#/', '') as Route;
  return validRoutes.includes(hash) ? hash : 'home';
}

export function navigate(route: Route): void {
  window.location.hash = `#/${route}`;
}

export function onRouteChange(fn: (route: Route) => void): void {
  window.addEventListener('hashchange', () => fn(currentRoute()));
}
