// Pantalla de inicio: elegir/abrir carpeta, correr init/prune/burst-detect,
// y los controles de deshacer/reiniciar agregados por feedback de uso real
// (ver docs/fase3-torneo.md, "Deshacer / reiniciar"). Envuelve exactamente
// los comandos del CLI, sin lógica nueva.
import { invoke } from '@tauri-apps/api/core';
import { cli, CliError } from '../api';
import { getProject, setProject, dbPathFor } from '../state';
import { showToast } from '../toast';
import { navigate } from '../router';
import { withLoadingOverlay } from '../components/LoadingOverlay';

const INIT_PHASES = [
  'Escaneando la carpeta…',
  'Extrayendo miniaturas…',
  'Calculando pHash…',
  'Calculando métricas de calidad…',
  'Emparejando RAW+JPEG del mismo disparo…',
];

export async function renderHome(container: HTMLElement): Promise<void> {
  const project = getProject();

  container.innerHTML = `
    <div class="view">
      <h1>Proyecto</h1>
      <div class="panel">
        <div class="field">
          <label>Carpeta de fotos</label>
          <div style="display:flex; gap:8px;">
            <input type="text" id="folder-input" placeholder="C:\\Fotos\\Boda_Juan" value="${
              project?.folderPath ?? ''
            }" style="flex:1" />
            <button class="btn" id="pick-folder-btn">Elegir…</button>
          </div>
        </div>
        <div style="height:12px"></div>
        <button class="btn btn-primary" id="init-btn">Inicializar / Actualizar (init)</button>
      </div>

      ${
        project
          ? `
      <div class="panel">
        <h2>Acciones sobre <span class="mono">${project.folderPath}</span></h2>
        <div class="panel-row" style="flex-wrap:wrap; gap:8px;">
          <button class="btn" id="prune-btn">prune</button>
          <button class="btn" id="burst-detect-btn">burst-detect</button>
          <button class="btn btn-primary" id="goto-bursts-btn">Ir a ráfagas &rarr;</button>
          <button class="btn btn-primary" id="goto-tournament-btn">Ir a torneo &rarr;</button>
        </div>
      </div>
      <div class="panel" id="result-panel" style="display:none"></div>

      <div class="panel">
        <h2>Deshacer / reiniciar torneo</h2>
        <p>Por si te equivocaste al mandar un grupo, o querés volver a empezar el torneo de esta carpeta desde cero.</p>
        <div class="panel-row" style="flex-wrap:wrap; gap:8px;">
          <button class="btn" id="undo-btn">tournament-undo (deshacer último grupo)</button>
          <button class="btn btn-danger" id="reset-tournament-btn">tournament-reset (reiniciar esta carpeta)</button>
        </div>
      </div>

      <div class="panel">
        <h2>Índice global</h2>
        <p>Vacía por completo <code>global_index.sqlite</code> — afecta el cálculo de estrellas de <strong>todas</strong> tus carpetas, no solo esta. Úsalo solo si sabés lo que hace.</p>
        <button class="btn btn-danger" id="reset-global-btn">reset-global-index</button>
      </div>
      `
          : ''
      }
    </div>
  `;

  const folderInput = container.querySelector<HTMLInputElement>('#folder-input')!;
  const resultPanel = container.querySelector<HTMLElement>('#result-panel');

  function showResult(title: string, data: unknown) {
    if (!resultPanel) return;
    resultPanel.style.display = 'block';
    resultPanel.innerHTML = `<h3>${title}</h3><pre class="mono" style="white-space:pre-wrap">${JSON.stringify(
      data,
      null,
      2,
    )}</pre>`;
  }

  container.querySelector('#pick-folder-btn')?.addEventListener('click', async () => {
    const picked = await invoke<string | null>('pick_folder');
    if (picked) folderInput.value = picked;
  });

  container.querySelector('#init-btn')?.addEventListener('click', async () => {
    const folder = folderInput.value.trim();
    if (!folder) {
      showToast('Elegí una carpeta primero', true);
      return;
    }
    try {
      const data = await withLoadingOverlay('Inicializando carpeta…', INIT_PHASES, () =>
        cli.init(folder),
      );
      setProject({ folderPath: folder, dbPath: dbPathFor(folder) });
      const pairedNote = data.paired_raw_jpeg > 0 ? `, ${data.paired_raw_jpeg} pares RAW+JPEG fusionados` : '';
      showToast(
        `init: ${data.inserted_ok} nuevas, ${data.skipped_existing} ya existentes, ${data.inserted_failed} fallidas${pairedNote}`,
      );
      renderHome(container);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector('#prune-btn')?.addEventListener('click', async () => {
    if (!project) return;
    try {
      const data = await cli.prune(project.dbPath);
      showResult('prune', data);
      showToast(`prune: ${data.marked_missing} marcadas como missing`);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector('#burst-detect-btn')?.addEventListener('click', async () => {
    if (!project) return;
    try {
      const data = await cli.burstDetect(project.dbPath);
      showResult('burst-detect', data);
      showToast(`burst-detect: ${data.bursts_created} ráfagas creadas`);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector('#undo-btn')?.addEventListener('click', async () => {
    if (!project) return;
    try {
      const data = await cli.tournamentUndo(project.dbPath);
      showResult('tournament-undo', data);
      showToast(`Deshecho: grupo ${data.group_id.slice(0, 8)}… (${data.reverted_images.length} imágenes)`);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector('#reset-tournament-btn')?.addEventListener('click', async () => {
    if (!project) return;
    if (!confirm(`¿Reiniciar el torneo de "${project.folderPath}"? mu/sigma vuelven al default en todas las imágenes activas. No afecta las decisiones de ráfaga (rejected).`)) {
      return;
    }
    try {
      const data = await cli.tournamentReset(project.dbPath);
      showResult('tournament-reset', data);
      showToast(`tournament-reset: ${data.images_reset} imágenes reiniciadas`);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector('#reset-global-btn')?.addEventListener('click', async () => {
    if (!confirm('¿Vaciar el índice global completo? Afecta el cálculo de estrellas de TODAS tus carpetas hasta que vuelvan a sincronizarse. Esta acción no se puede deshacer.')) {
      return;
    }
    try {
      const data = await cli.resetGlobalIndex();
      showResult('reset-global-index', data);
      showToast(`reset-global-index: ${data.rows_deleted} filas eliminadas`);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector('#goto-bursts-btn')?.addEventListener('click', () => navigate('bursts'));
  container
    .querySelector('#goto-tournament-btn')
    ?.addEventListener('click', () => navigate('tournament'));
}
