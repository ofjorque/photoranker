// Minitorneo de ráfagas — ver docs/fase1-ingesta.md, "Detección y minitorneo
// de ráfagas". Misma mecánica de teclado que el torneo principal (RankingBoard).
import { cli, CliError } from '../api';
import type { PendingBurst } from '../api/types';
import { getProject } from '../state';
import { showToast } from '../toast';
import { mountRankingBoard } from '../components/RankingBoard';

export async function renderBursts(container: HTMLElement): Promise<() => void> {
  let boardDestroy: (() => void) | null = null;
  const cleanup = () => boardDestroy?.();

  async function load() {
    boardDestroy?.();
    boardDestroy = null;

    const project = getProject();
    if (!project) {
      container.innerHTML =
        '<div class="view"><div class="empty-state">Abrí un proyecto primero.</div></div>';
      return;
    }

    container.innerHTML = '<div class="view"><p>Cargando ráfagas pendientes…</p></div>';

    let bursts: PendingBurst[];
    try {
      bursts = await cli.listBursts(project.dbPath);
    } catch (e) {
      container.innerHTML = `<div class="view"><div class="empty-state">${
        e instanceof CliError ? e.message : String(e)
      }</div></div>`;
      return;
    }

    if (bursts.length === 0) {
      container.innerHTML = `
        <div class="view">
          <h1>Ráfagas</h1>
          <div class="empty-state">No hay ráfagas pendientes de minitorneo. Corré <code>burst-detect</code> en la pantalla de Proyecto si agregaste fotos nuevas.</div>
        </div>`;
      return;
    }

    const burst = bursts[0];

    container.innerHTML = '';
    const heading = document.createElement('div');
    heading.className = 'view';
    heading.innerHTML = `<h1>Ráfaga #${burst.id} <span style="color:var(--color-text-muted); font-weight:400">(${bursts.length} pendiente${bursts.length === 1 ? '' : 's'})</span></h1>
      <p>Ordená de mejor a peor con el teclado. La ganadora (posición 1) se conserva; el resto queda marcado como <code>rejected</code>.</p>`;
    container.appendChild(heading);

    const boardContainer = document.createElement('div');
    container.appendChild(boardContainer);

    const board = mountRankingBoard(boardContainer, {
      dbPath: project.dbPath,
      images: burst.images,
      onSubmit: async (ranking) => {
        try {
          const result = await cli.burstTournament(project.dbPath, burst.id, ranking);
          showToast(
            `Burst #${burst.id}: ganadora ${result.representative_image_id}, ${result.rejected} rechazadas`,
          );
          await load();
        } catch (e) {
          showToast(e instanceof CliError ? e.message : String(e), true);
        }
      },
    });
    boardDestroy = board.destroy;
  }

  await load();
  return cleanup;
}
