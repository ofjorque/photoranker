// Minitorneo de ráfagas — ver docs/fase1-ingesta.md, "Detección y minitorneo
// de ráfagas". Misma mecánica de teclado que el torneo principal (RankingBoard).
import { cli, CliError } from '../api';
import type { PendingBurst, ResolvedBurst } from '../api/types';
import { getProject } from '../state';
import { showToast } from '../toast';
import { mountRankingBoard } from '../components/RankingBoard';
import { getThumbnailDataUrl } from '../api/thumbnailCache';

async function renderResolvedBurstsSection(container: HTMLElement, dbPath: string): Promise<void> {
  let resolved: ResolvedBurst[];
  try {
    resolved = await cli.listBurstsResolved(dbPath);
  } catch (e) {
    container.innerHTML = `<div class="empty-state">${e instanceof CliError ? e.message : String(e)}</div>`;
    return;
  }

  if (resolved.length === 0) {
    container.innerHTML = '<p>Todavía no resolviste ningún minitorneo de ráfaga.</p>';
    return;
  }

  container.innerHTML = '';
  for (const burst of resolved.slice(0, 10)) {
    const row = document.createElement('div');
    row.className = 'panel-nested panel-row';
    row.style.marginBottom = '8px';
    const label = document.createElement('span');
    label.textContent = `Ráfaga #${burst.id} — ganadora: imagen ${burst.representative_image_id ?? '?'} (${burst.images.length} fotos)`;
    row.appendChild(label);
    const undoBtn = document.createElement('button');
    undoBtn.className = 'btn btn-danger';
    undoBtn.textContent = 'Deshacer';
    undoBtn.addEventListener('click', async () => {
      try {
        await cli.burstUndo(dbPath, burst.id);
        showToast(`Ráfaga #${burst.id} deshecha — vuelve a estar pendiente`);
        await renderResolvedBurstsSection(container, dbPath);
      } catch (e) {
        showToast(e instanceof CliError ? e.message : String(e), true);
      }
    });
    row.appendChild(undoBtn);
    container.appendChild(row);
  }
}

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

    // Excluir imagen(es) del burst antes de resolverlo (feedback de uso
    // real: "esta imagen no es parte de una ráfaga") — solo se muestra si el
    // burst tiene más de 2 miembros, porque excluir hasta dejar 1 disuelve
    // el burst completo (ver burst_detect::exclude en el CLI).
    if (burst.images.length > 2) {
      const excludePanel = document.createElement('div');
      excludePanel.className = 'panel disclosure';
      excludePanel.style.marginTop = '12px';
      excludePanel.innerHTML = '<h3>¿Alguna de estas fotos no es parte de la ráfaga?</h3>';
      const grid = document.createElement('div');
      grid.style.display = 'grid';
      grid.style.gridTemplateColumns = 'repeat(auto-fill, minmax(96px, 1fr))';
      grid.style.gap = '8px';
      grid.style.margin = '10px 0';

      const selected = new Set<number>();
      for (const img of burst.images) {
        const wrap = document.createElement('label');
        wrap.style.display = 'flex';
        wrap.style.flexDirection = 'column';
        wrap.style.gap = '4px';
        wrap.style.cursor = 'pointer';
        const thumb = document.createElement('div');
        thumb.className = 'thumb-wrap';
        getThumbnailDataUrl(project.dbPath, img.id).then((url) => {
          if (!url) return;
          const el = document.createElement('img');
          el.src = url;
          el.style.width = '100%';
          el.style.height = '100%';
          el.style.objectFit = 'cover';
          thumb.appendChild(el);
        });
        const checkboxRow = document.createElement('div');
        checkboxRow.style.display = 'flex';
        checkboxRow.style.gap = '4px';
        checkboxRow.style.alignItems = 'center';
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.addEventListener('change', () => {
          if (checkbox.checked) selected.add(img.id);
          else selected.delete(img.id);
        });
        const smallLabel = document.createElement('span');
        smallLabel.style.fontSize = '11px';
        smallLabel.textContent = 'no es burst';
        checkboxRow.appendChild(checkbox);
        checkboxRow.appendChild(smallLabel);
        wrap.appendChild(thumb);
        wrap.appendChild(checkboxRow);
        grid.appendChild(wrap);
      }
      excludePanel.appendChild(grid);

      const excludeBtn = document.createElement('button');
      excludeBtn.className = 'btn';
      excludeBtn.textContent = 'Excluir seleccionadas';
      excludeBtn.addEventListener('click', async () => {
        if (selected.size === 0) {
          showToast('Seleccioná al menos una foto para excluir', true);
          return;
        }
        try {
          const result = await cli.burstExclude(project.dbPath, burst.id, Array.from(selected));
          showToast(
            result.burst_dissolved
              ? `Ráfaga #${burst.id} disuelta (quedaba solo 1 imagen)`
              : `${result.excluded.length} imagen(es) excluida(s) de la ráfaga #${burst.id}`,
          );
          await load();
        } catch (e) {
          showToast(e instanceof CliError ? e.message : String(e), true);
        }
      });
      excludePanel.appendChild(excludeBtn);
      container.appendChild(excludePanel);
    }

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

    // Sección "deshacer" — colapsada por defecto (divulgación progresiva,
    // feedback de uso real sobre que la GUI se ve básica): no tiene sentido
    // mostrar el historial de bursts resueltos hasta que el usuario lo pida.
    const resolvedSection = document.createElement('div');
    resolvedSection.className = 'view';
    resolvedSection.innerHTML = `
      <div class="panel">
        <div class="panel-row" style="cursor:pointer" id="resolved-toggle">
          <h2>Bursts ya resueltos (deshacer)</h2>
          <span class="badge badge-muted">mostrar/ocultar</span>
        </div>
        <div id="resolved-body" style="display:none; margin-top:12px"></div>
      </div>
    `;
    container.appendChild(resolvedSection);
    const resolvedBody = resolvedSection.querySelector<HTMLElement>('#resolved-body')!;
    let resolvedLoaded = false;
    resolvedSection.querySelector('#resolved-toggle')?.addEventListener('click', async () => {
      const isHidden = resolvedBody.style.display === 'none';
      resolvedBody.style.display = isHidden ? 'block' : 'none';
      if (isHidden) {
        resolvedBody.className = 'disclosure';
        if (!resolvedLoaded) {
          resolvedLoaded = true;
          await renderResolvedBurstsSection(resolvedBody, project.dbPath);
        }
      }
    });
  }

  await load();
  return cleanup;
}
