// Torneo principal — ver docs/fase3-torneo.md. tournament-next arma el
// grupo, RankingBoard captura el ranking por teclado, tournament-result lo
// envía. El panel de calidad (fase5-gui.md) sigue a la imagen en foco.
import { cli, CliError } from '../api';
import { getProject } from '../state';
import { showToast } from '../toast';
import { mountRankingBoard } from '../components/RankingBoard';
import { renderQualityPanel } from '../components/QualityPanel';

export async function renderTournament(container: HTMLElement): Promise<() => void> {
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

    container.innerHTML = '<div class="view"><p>Cargando estado del torneo…</p></div>';

    const [status, group] = await Promise.all([
      cli.tournamentStatus(project.dbPath).catch(() => null),
      cli.tournamentNext(project.dbPath).catch((e) => {
        showToast(e instanceof CliError ? e.message : String(e), true);
        return null;
      }),
    ]);

    container.innerHTML = '';

    const view = document.createElement('div');
    view.className = 'view';

    const statusPanel = document.createElement('div');
    statusPanel.className = 'panel';
    if (status) {
      statusPanel.innerHTML = `
        <h2>Progreso del torneo</h2>
        <div class="stat-grid">
          <div class="stat-tile"><div class="label">Activas</div><div class="value">${status.active_images}</div></div>
          <div class="stat-tile"><div class="label">Convergidas</div><div class="value">${status.converged_images}</div></div>
          <div class="stat-tile"><div class="label">Estancadas</div><div class="value">${status.stalled_images}</div></div>
          <div class="stat-tile"><div class="label">Rondas</div><div class="value">${status.rounds_completed}/${status.max_rounds}</div></div>
          <div class="stat-tile"><div class="label">% Convergencia</div><div class="value">${(status.convergence_ratio * 100).toFixed(0)}%</div></div>
          <div class="stat-tile"><div class="label">Estado</div><div class="value badge ${
            status.status === 'converged' ? 'badge-success' : 'badge-muted'
          }">${status.status}</div></div>
        </div>`;
    } else {
      statusPanel.innerHTML = '<p>No se pudo leer tournament-status.</p>';
    }
    view.appendChild(statusPanel);

    if (!group) {
      const empty = document.createElement('div');
      empty.className = 'empty-state';
      empty.textContent =
        'No hay grupo disponible (menos de 2 imágenes activas, o el torneo ya convergió).';
      view.appendChild(empty);
      container.appendChild(view);
      return;
    }

    const heading = document.createElement('h1');
    heading.textContent = `Grupo ${group.group_id.slice(0, 8)}…`;
    view.appendChild(heading);

    const layout = document.createElement('div');
    layout.style.display = 'grid';
    layout.style.gridTemplateColumns = '1fr 260px';
    layout.style.gap = '24px';
    layout.style.alignItems = 'start';

    const boardCol = document.createElement('div');
    const qualityCol = document.createElement('div');
    qualityCol.className = 'panel';
    qualityCol.innerHTML = '<h3>Panel de calidad</h3><p>Enfocá una imagen…</p>';

    layout.appendChild(boardCol);
    layout.appendChild(qualityCol);
    view.appendChild(layout);
    container.appendChild(view);

    const board = mountRankingBoard(boardCol, {
      dbPath: project.dbPath,
      images: group.images,
      captionFor: (img) => {
        const found = group.images.find((i) => i.id === img.id);
        return found ? `μ=${found.mu.toFixed(1)} σ=${found.sigma.toFixed(1)}` : '';
      },
      onFocusChange: (img) => {
        qualityCol.innerHTML = '<h3>Panel de calidad</h3><div class="quality-body"></div>';
        const target = qualityCol.querySelector('.quality-body') as HTMLElement;
        renderQualityPanel(target, project.dbPath, img.id);
      },
      onSubmit: async (ranking) => {
        try {
          await cli.tournamentResult(project.dbPath, group.group_id, ranking);
          showToast('Resultado enviado');
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
