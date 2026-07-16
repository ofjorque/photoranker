// Clustering — ver docs/fase2-clustering.md. --preview grafica el scree plot
// de BIC; --k <N> compromete resultados; cluster-rename los bautiza (ahora
// ayudado por unas pocas fotos representativas de cada cluster, ver
// docs/fase5-gui.md, agregado por feedback de uso real).
import { cli, CliError } from '../api';
import type { ClusterSummary } from '../api/types';
import { getProject } from '../state';
import { showToast } from '../toast';
import { renderScreePlot } from '../components/ScreePlot';
import { getThumbnailDataUrl } from '../api/thumbnailCache';
import { withLoadingOverlay } from '../components/LoadingOverlay';

const PREVIEW_PHASES = [
  'Invocando Rscript…',
  'Ajustando clustMD para cada k…',
  'Probando modelos de covarianza (EII/VII/EEI/VEI)…',
  'Calculando BIC…',
];

const COMMIT_PHASES = [
  'Invocando Rscript…',
  'Ajustando el modelo de mezcla (EM)…',
  'Asignando cada imagen a su cluster…',
  'Guardando resultados…',
];

async function renderClustersList(container: HTMLElement, dbPath: string): Promise<void> {
  container.innerHTML = '<p>Cargando clusters…</p>';
  let clusters: ClusterSummary[];
  try {
    clusters = await cli.listClusters(dbPath);
  } catch (e) {
    container.innerHTML = `<div class="empty-state">${
      e instanceof CliError ? e.message : String(e)
    }</div>`;
    return;
  }

  if (clusters.length === 0) {
    container.innerHTML =
      '<div class="empty-state">Todavía no hay clusters comprometidos — corré <code>cluster --k</code> arriba.</div>';
    return;
  }

  container.innerHTML = '';
  const grid = document.createElement('div');
  grid.style.display = 'grid';
  grid.style.gridTemplateColumns = 'repeat(auto-fill, minmax(240px, 1fr))';
  grid.style.gap = '16px';

  for (const cluster of clusters) {
    const card = document.createElement('div');
    card.className = 'panel';
    card.style.padding = '12px';

    const header = document.createElement('div');
    header.className = 'panel-row';
    header.innerHTML = `<strong>${cluster.name ?? `Cluster ${cluster.id}`}</strong><span class="badge badge-muted">${cluster.member_count} fotos</span>`;
    card.appendChild(header);

    const thumbRow = document.createElement('div');
    thumbRow.style.display = 'grid';
    thumbRow.style.gridTemplateColumns = `repeat(${cluster.representative_images.length || 1}, 1fr)`;
    thumbRow.style.gap = '6px';
    thumbRow.style.margin = '10px 0';

    for (const img of cluster.representative_images) {
      const thumbWrap = document.createElement('div');
      thumbWrap.style.aspectRatio = '1';
      thumbWrap.style.borderRadius = 'var(--radius-sm)';
      thumbWrap.style.overflow = 'hidden';
      thumbWrap.style.background = 'var(--color-bg)';
      thumbRow.appendChild(thumbWrap);
      getThumbnailDataUrl(dbPath, img.id).then((url) => {
        if (!url) return;
        const el = document.createElement('img');
        el.src = url;
        el.style.width = '100%';
        el.style.height = '100%';
        el.style.objectFit = 'cover';
        thumbWrap.appendChild(el);
      });
    }
    card.appendChild(thumbRow);

    const renameRow = document.createElement('div');
    renameRow.style.display = 'flex';
    renameRow.style.gap = '6px';
    const nameInput = document.createElement('input');
    nameInput.type = 'text';
    nameInput.placeholder = 'Nombre del cluster';
    nameInput.value = cluster.name ?? '';
    nameInput.style.flex = '1';
    const renameBtn = document.createElement('button');
    renameBtn.className = 'btn';
    renameBtn.textContent = 'Renombrar';
    renameBtn.addEventListener('click', async () => {
      const name = nameInput.value.trim();
      if (!name) {
        showToast('El nombre no puede estar vacío', true);
        return;
      }
      try {
        await cli.clusterRename(dbPath, cluster.id, name);
        showToast(`Cluster ${cluster.id} renombrado a "${name}"`);
        await renderClustersList(container, dbPath);
      } catch (e) {
        showToast(e instanceof CliError ? e.message : String(e), true);
      }
    });
    renameRow.appendChild(nameInput);
    renameRow.appendChild(renameBtn);
    card.appendChild(renameRow);

    grid.appendChild(card);
  }

  container.appendChild(grid);
}

export async function renderCluster(container: HTMLElement): Promise<void> {
  const project = getProject();
  if (!project) {
    container.innerHTML =
      '<div class="view"><div class="empty-state">Abrí un proyecto primero.</div></div>';
    return;
  }
  const dbPath = project.dbPath;

  container.innerHTML = `
    <div class="view">
      <h1>Clustering (clustMD)</h1>
      <div class="panel">
        <div class="panel-row">
          <h2>Vista previa (BIC por k)</h2>
          <button class="btn" id="preview-btn">cluster --preview</button>
        </div>
        <div id="scree-plot"></div>
      </div>

      <div class="panel">
        <h2>Comprometer clustering</h2>
        <div style="display:flex; gap:8px; align-items:end;">
          <div class="field">
            <label>k (vacío = automático, mejor BIC)</label>
            <input type="number" id="k-input" min="2" max="10" />
          </div>
          <button class="btn btn-primary" id="commit-btn">cluster --k</button>
        </div>
        <div id="commit-result" style="margin-top:12px"></div>
      </div>

      <div class="panel">
        <h2>Clusters — fotos representativas para elegir el nombre</h2>
        <div id="clusters-list"></div>
      </div>
    </div>
  `;

  const screeContainer = container.querySelector<HTMLElement>('#scree-plot')!;
  const clustersListContainer = container.querySelector<HTMLElement>('#clusters-list')!;

  await renderClustersList(clustersListContainer, dbPath);

  container.querySelector('#preview-btn')?.addEventListener('click', async () => {
    try {
      const data = await withLoadingOverlay('Corriendo clustMD (R)…', PREVIEW_PHASES, () =>
        cli.clusterPreview(dbPath),
      );
      renderScreePlot(screeContainer, data.bic_by_k);
    } catch (e) {
      screeContainer.innerHTML = `<div class="empty-state">${
        e instanceof CliError ? e.message : String(e)
      }</div>`;
    }
  });

  const commitResult = container.querySelector<HTMLElement>('#commit-result')!;
  container.querySelector('#commit-btn')?.addEventListener('click', async () => {
    const kInput = container.querySelector<HTMLInputElement>('#k-input')!;
    const k = kInput.value.trim() === '' ? undefined : Number(kInput.value);
    try {
      const data = await withLoadingOverlay('Corriendo clustMD (R)…', COMMIT_PHASES, () =>
        cli.clusterCommit(dbPath, k),
      );
      commitResult.innerHTML = `<pre class="mono" style="white-space:pre-wrap">${JSON.stringify(
        data,
        null,
        2,
      )}</pre>`;
      showToast('Clustering comprometido');
      await renderClustersList(clustersListContainer, dbPath);
    } catch (e) {
      const msg = e instanceof CliError ? e.message : String(e);
      commitResult.innerHTML = `<div class="empty-state">${msg}</div>`;
      showToast(msg, true);
    }
  });
}
