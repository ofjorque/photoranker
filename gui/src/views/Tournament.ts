// Torneo principal — ver docs/fase3-torneo.md. tournament-next arma el
// grupo, RankingBoard captura el ranking por teclado, tournament-result lo
// envía. El panel de calidad (fase5-gui.md) sigue a la imagen en foco.
import { cli, CliError } from '../api';
import { getProject } from '../state';
import { showToast } from '../toast';
import { mountRankingBoard } from '../components/RankingBoard';
import { renderQualityPanel } from '../components/QualityPanel';
import { icons } from '../components/icons';
import { t } from '../i18n';

const QUALITY_PANEL_COLLAPSED_KEY = 'photoranker-quality-panel-collapsed';

export async function renderTournament(container: HTMLElement): Promise<() => void> {
  let boardDestroy: (() => void) | null = null;
  const cleanup = () => boardDestroy?.();

  async function load() {
    boardDestroy?.();
    boardDestroy = null;

    const project = getProject();
    if (!project) {
      container.innerHTML = `<div class="view"><div class="empty-state">${t('common.openProjectFirst')}</div></div>`;
      return;
    }

    container.innerHTML = `<div class="view"><p>${t('tournament.loadingStatus')}</p></div>`;

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
        <h2>${t('tournament.status.title')}</h2>
        <div class="stat-grid">
          <div class="stat-tile"><div class="label">${t('tournament.status.active')}</div><div class="value">${status.active_images}</div></div>
          <div class="stat-tile"><div class="label">${t('tournament.status.converged')}</div><div class="value">${status.converged_images}</div></div>
          <div class="stat-tile"><div class="label">${t('tournament.status.stalled')}</div><div class="value">${status.stalled_images}</div></div>
          <div class="stat-tile"><div class="label">${t('tournament.status.rounds')}</div><div class="value">${status.rounds_completed}/${status.max_rounds}</div></div>
          <div class="stat-tile"><div class="label">${t('tournament.status.convergencePct')}</div><div class="value">${(status.convergence_ratio * 100).toFixed(0)}%</div></div>
          <div class="stat-tile"><div class="label">${t('tournament.status.state')}</div><div class="value badge ${
            status.status === 'converged' ? 'badge-success' : 'badge-muted'
          }">${status.status}</div></div>
        </div>`;
    } else {
      statusPanel.innerHTML = `<p>${t('tournament.status.loadError')}</p>`;
    }
    view.appendChild(statusPanel);

    if (!group) {
      const empty = document.createElement('div');
      empty.className = 'empty-state';
      empty.textContent = t('tournament.noGroup');
      view.appendChild(empty);
      container.appendChild(view);
      return;
    }

    const heading = document.createElement('h1');
    heading.textContent = t('tournament.groupHeading', { groupId: group.group_id.slice(0, 8) });
    view.appendChild(heading);

    // Panel colapsable (estilo Darktable: los grupos de módulos laterales se
    // pueden achicar para dejar más espacio a la imagen) — el estado se
    // persiste en localStorage, es una preferencia de esta instalación, no
    // del proyecto/carpeta.
    const collapsed = localStorage.getItem(QUALITY_PANEL_COLLAPSED_KEY) === 'true';

    const layout = document.createElement('div');
    layout.className = 'tournament-layout' + (collapsed ? ' tournament-layout--collapsed' : '');

    const boardCol = document.createElement('div');
    const qualityCol = document.createElement('div');
    qualityCol.className = 'panel tournament-quality-col';

    const qualityHeader = document.createElement('div');
    qualityHeader.className = 'panel-row';
    const qualityToggle = document.createElement('button');
    qualityToggle.className = 'quality-collapse-btn';
    qualityToggle.title = t('tournament.quality.toggleTitle');
    qualityToggle.innerHTML = collapsed ? icons.chevronLeft : icons.chevronRight;
    qualityToggle.addEventListener('click', () => {
      const nowCollapsed = layout.classList.toggle('tournament-layout--collapsed');
      localStorage.setItem(QUALITY_PANEL_COLLAPSED_KEY, String(nowCollapsed));
      qualityToggle.innerHTML = nowCollapsed ? icons.chevronLeft : icons.chevronRight;
    });
    qualityHeader.appendChild(document.createElement('h3')).textContent = t('tournament.quality.title');
    qualityHeader.appendChild(qualityToggle);
    qualityCol.appendChild(qualityHeader);
    const qualityBodyWrap = document.createElement('div');
    qualityBodyWrap.className = 'quality-body';
    qualityBodyWrap.innerHTML = `<p>${t('tournament.quality.focusHint')}</p>`;
    qualityCol.appendChild(qualityBodyWrap);

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
        renderQualityPanel(qualityBodyWrap, project.dbPath, img.id);
      },
      onSubmit: async (ranking) => {
        try {
          await cli.tournamentResult(project.dbPath, group.group_id, ranking);
          showToast(t('tournament.resultSent'));
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
