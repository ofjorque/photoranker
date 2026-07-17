// Ranking en vivo + exportación a XMP + miniaturas fallidas — ver
// docs/fase3-torneo.md (`ranking`) y docs/fase4-exportacion.md (`export-xmp`,
// `list-failed-thumbnails`, `retry-thumbnail`).
import { cli, CliError } from '../api';
import { getProject } from '../state';
import { showToast } from '../toast';
import { showLoadingOverlay } from '../components/LoadingOverlay';
import { t } from '../i18n';

function exportPhases(): string[] {
  return [t('export.phase.reading'), t('export.phase.stars'), t('export.phase.writing')];
}

export async function renderExport(container: HTMLElement): Promise<void> {
  const project = getProject();
  if (!project) {
    container.innerHTML = `<div class="view"><div class="empty-state">${t('common.openProjectFirst')}</div></div>`;
    return;
  }
  const dbPath = project.dbPath;

  container.innerHTML = `
    <div class="view">
      <h1>${t('export.title')}</h1>

      <div class="panel">
        <div class="panel-row">
          <h2>${t('export.liveRanking.title')}</h2>
          <button class="btn" id="refresh-ranking-btn">${t('common.refresh')}</button>
        </div>
        <div id="ranking-table"><p>${t('common.loading')}</p></div>
      </div>

      <div class="panel">
        <div class="panel-row">
          <h2>${t('export.failedThumbnails.title')}</h2>
          <button class="btn" id="refresh-failed-btn">${t('common.refresh')}</button>
        </div>
        <div id="failed-table"><p>${t('common.loading')}</p></div>
      </div>

      <div class="panel">
        <h2>${t('export.xmp.title')}</h2>
        <p>${t('export.xmp.description')}</p>
        <button class="btn btn-primary" id="export-btn">export-xmp</button>
        <div id="export-result" style="margin-top:12px"></div>
      </div>
    </div>
  `;

  const rankingTable = container.querySelector<HTMLElement>('#ranking-table')!;
  const failedTable = container.querySelector<HTMLElement>('#failed-table')!;
  const exportResult = container.querySelector<HTMLElement>('#export-result')!;

  async function loadRanking() {
    rankingTable.innerHTML = `<p>${t('common.loading')}</p>`;
    try {
      const rows = await cli.ranking(dbPath);
      if (rows.length === 0) {
        rankingTable.innerHTML = `<div class="empty-state">${t('export.liveRanking.empty')}</div>`;
        return;
      }
      rankingTable.innerHTML = `
        <table>
          <thead><tr><th>#</th><th>${t('export.liveRanking.colFile')}</th><th>μ</th><th>σ</th><th>${t('export.liveRanking.colState')}</th></tr></thead>
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
                        : `<span class="badge badge-success">${t('export.liveRanking.active')}</span>`
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
    failedTable.innerHTML = `<p>${t('common.loading')}</p>`;
    try {
      const rows = await cli.listFailedThumbnails(dbPath);
      if (rows.length === 0) {
        failedTable.innerHTML = `<div class="empty-state">${t('export.failedThumbnails.empty')}</div>`;
        return;
      }
      failedTable.innerHTML = `
        <table>
          <thead><tr><th>ID</th><th>${t('export.liveRanking.colFile')}</th><th></th></tr></thead>
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
            showToast(t('export.failedThumbnails.retried', { id }));
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
  container.querySelector<HTMLButtonElement>('#export-btn')?.addEventListener('click', async () => {
    // Diálogo explícito de progreso + confirmación de "listo" (feedback de
    // uso real: "debería haber un diálogo que muestre el trabajo que se está
    // haciendo y que diga que todo finalizó") — no un simple toast que
    // desaparece solo, ver LoadingHandle.finish en LoadingOverlay.ts.
    exportResult.textContent = '';
    const handle = showLoadingOverlay(t('export.xmp.loadingTitle'), exportPhases());
    try {
      const data = await cli.exportXmp(dbPath);
      exportResult.innerHTML = `<pre class="mono" style="white-space:pre-wrap">${JSON.stringify(
        data,
        null,
        2,
      )}</pre>`;
      await handle.finish(
        t('export.xmp.result', {
          written: data.written,
          failedThumbnail: data.excluded_failed_thumbnail,
          missing: data.excluded_missing,
        }),
      );
    } catch (e) {
      const msg = e instanceof CliError ? e.message : String(e);
      exportResult.innerHTML = `<div class="empty-state">${msg}</div>`;
      handle.close();
      showToast(msg, true);
    }
  });

  await Promise.all([loadRanking(), loadFailed()]);
}
