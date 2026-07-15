// Clustering — ver docs/fase2-clustering.md. --preview grafica el scree plot
// de BIC; --k <N> compromete resultados; cluster-rename los bautiza.
import { cli, CliError } from '../api';
import { getProject } from '../state';
import { showToast } from '../toast';
import { renderScreePlot } from '../components/ScreePlot';

export async function renderCluster(container: HTMLElement): Promise<void> {
  const project = getProject();
  if (!project) {
    container.innerHTML =
      '<div class="view"><div class="empty-state">Abrí un proyecto primero.</div></div>';
    return;
  }

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
        <h2>Renombrar cluster</h2>
        <div style="display:flex; gap:8px; align-items:end;">
          <div class="field">
            <label>ID</label>
            <input type="number" id="rename-id-input" style="width:100px" />
          </div>
          <div class="field" style="flex:1">
            <label>Nombre</label>
            <input type="text" id="rename-name-input" placeholder="Retratos nocturnos" />
          </div>
          <button class="btn" id="rename-btn">cluster-rename</button>
        </div>
      </div>
    </div>
  `;

  const screeContainer = container.querySelector<HTMLElement>('#scree-plot')!;

  container.querySelector('#preview-btn')?.addEventListener('click', async () => {
    screeContainer.innerHTML = '<p>Corriendo clustMD (R)…</p>';
    try {
      const data = await cli.clusterPreview(project.dbPath);
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
    commitResult.textContent = 'Corriendo clustMD (R)…';
    try {
      const data = await cli.clusterCommit(project.dbPath, k);
      commitResult.innerHTML = `<pre class="mono" style="white-space:pre-wrap">${JSON.stringify(
        data,
        null,
        2,
      )}</pre>`;
      showToast('Clustering comprometido');
    } catch (e) {
      const msg = e instanceof CliError ? e.message : String(e);
      commitResult.innerHTML = `<div class="empty-state">${msg}</div>`;
      showToast(msg, true);
    }
  });

  container.querySelector('#rename-btn')?.addEventListener('click', async () => {
    const idInput = container.querySelector<HTMLInputElement>('#rename-id-input')!;
    const nameInput = container.querySelector<HTMLInputElement>('#rename-name-input')!;
    const id = Number(idInput.value);
    const name = nameInput.value.trim();
    if (!id || !name) {
      showToast('Completá id y nombre', true);
      return;
    }
    try {
      await cli.clusterRename(project.dbPath, id, name);
      showToast(`Cluster ${id} renombrado a "${name}"`);
      nameInput.value = '';
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });
}
