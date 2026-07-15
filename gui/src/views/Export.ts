// Ranking en vivo + exportación a XMP + miniaturas fallidas — ver
// docs/fase3-torneo.md (`ranking`) y docs/fase4-exportacion.md (`export-xmp`,
// `list-failed-thumbnails`, `retry-thumbnail`).
import { cli, CliError } from '../api';
import { getProject } from '../state';
import { showToast } from '../toast';

export async function renderExport(container: HTMLElement): Promise<void> {
  const project = getProject();
  if (!project) {
    container.innerHTML =
      '<div class="view"><div class="empty-state">Abrí un proyecto primero.</div></div>';
    return;
  }
  const dbPath = project.dbPath;

  container.innerHTML = `
    <div class="view">
      <h1>Ranking &amp; Exportación</h1>

      <div class="panel">
        <div class="panel-row">
          <h2>Ranking en vivo</h2>
          <button class="btn" id="refresh-ranking-btn">Actualizar</button>
        </div>
        <div id="ranking-table"><p>Cargando…</p></div>
      </div>

      <div class="panel">
        <div class="panel-row">
          <h2>Miniaturas fallidas</h2>
          <button class="btn" id="refresh-failed-btn">Actualizar</button>
        </div>
        <div id="failed-table"><p>Cargando…</p></div>
      </div>

      <div class="panel">
        <h2>Exportar a XMP</h2>
        <p>Escribe sidecars <code>.xmp</code> junto a cada foto (convención Darktable), de forma no destructiva.</p>
        <button class="btn btn-primary" id="export-btn">export-xmp</button>
        <div id="export-result" style="margin-top:12px"></div>
      </div>
    </div>
  `;

  const rankingTable = container.querySelector<HTMLElement>('#ranking-table')!;
  const failedTable = container.querySelector<HTMLElement>('#failed-table')!;
  const exportResult = container.querySelector<HTMLElement>('#export-result')!;

  async function loadRanking() {
    rankingTable.innerHTML = '<p>Cargando…</p>';
    try {
      const rows = await cli.ranking(dbPath);
      if (rows.length === 0) {
        rankingTable.innerHTML = '<div class="empty-state">Sin imágenes activas.</div>';
        return;
      }
      rankingTable.innerHTML = `
        <table>
          <thead><tr><th>#</th><th>Archivo</th><th>μ</th><th>σ</th><th>Estado</th></tr></thead>
          <tbody>
            ${rows
              .map(
                (r, i) => `<tr>
                  <td class="mono">${i + 1}</td>
                  <td title="${r.file_path}">${r.file_path.split(/[\\/]/).pop()}</td>
                  <td class="mono">${r.mu.toFixed(2)}</td>
                  <td class="mono">${r.sigma.toFixed(2)}</td>
                  <td>${
                    r.rejected
                      ? '<span class="badge badge-danger">rejected</span>'
                      : r.stalled
                        ? '<span class="badge badge-muted">stalled</span>'
                        : '<span class="badge badge-success">activa</span>'
                  }</td>
                </tr>`,
              )
              .join('')}
          </tbody>
        </table>`;
    } catch (e) {
      rankingTable.innerHTML = `<div class="empty-state">${
        e instanceof CliError ? e.message : String(e)
      }</div>`;
    }
  }

  async function loadFailed() {
    failedTable.innerHTML = '<p>Cargando…</p>';
    try {
      const rows = await cli.listFailedThumbnails(dbPath);
      if (rows.length === 0) {
        failedTable.innerHTML = '<div class="empty-state">Ninguna — todas las miniaturas se extrajeron bien.</div>';
        return;
      }
      failedTable.innerHTML = `
        <table>
          <thead><tr><th>ID</th><th>Archivo</th><th></th></tr></thead>
          <tbody>
            ${rows
              .map(
                (r) => `<tr>
                  <td class="mono">${r.id}</td>
                  <td title="${r.file_path}">${r.file_path.split(/[\\/]/).pop()}</td>
                  <td><button class="btn" data-retry-id="${r.id}">retry-thumbnail</button></td>
                </tr>`,
              )
              .join('')}
          </tbody>
        </table>`;
      failedTable.querySelectorAll<HTMLButtonElement>('[data-retry-id]').forEach((btn) => {
        btn.addEventListener('click', async () => {
          const id = Number(btn.dataset.retryId);
          try {
            await cli.retryThumbnail(dbPath, id);
            showToast(`Imagen ${id}: miniatura recuperada`);
            await loadFailed();
          } catch (e) {
            showToast(e instanceof CliError ? e.message : String(e), true);
          }
        });
      });
    } catch (e) {
      failedTable.innerHTML = `<div class="empty-state">${
        e instanceof CliError ? e.message : String(e)
      }</div>`;
    }
  }

  container.querySelector('#refresh-ranking-btn')?.addEventListener('click', loadRanking);
  container.querySelector('#refresh-failed-btn')?.addEventListener('click', loadFailed);
  container.querySelector('#export-btn')?.addEventListener('click', async () => {
    exportResult.textContent = 'Exportando…';
    try {
      const data = await cli.exportXmp(dbPath);
      exportResult.innerHTML = `<pre class="mono" style="white-space:pre-wrap">${JSON.stringify(
        data,
        null,
        2,
      )}</pre>`;
      showToast(`export-xmp: ${data.written} sidecars escritos`);
    } catch (e) {
      const msg = e instanceof CliError ? e.message : String(e);
      exportResult.innerHTML = `<div class="empty-state">${msg}</div>`;
      showToast(msg, true);
    }
  });

  await Promise.all([loadRanking(), loadFailed()]);
}
