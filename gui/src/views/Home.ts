// Pantalla de inicio: elegir/abrir carpeta, correr init/prune/burst-detect.
// Envuelve exactamente los comandos de fase1-ingesta.md, sin lógica nueva.
import { invoke } from '@tauri-apps/api/core';
import { cli, CliError } from '../api';
import { getProject, setProject, dbPathFor } from '../state';
import { showToast } from '../toast';
import { navigate } from '../router';

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
      const data = await cli.init(folder);
      setProject({ folderPath: folder, dbPath: dbPathFor(folder) });
      showToast(
        `init: ${data.inserted_ok} nuevas, ${data.skipped_existing} ya existentes, ${data.inserted_failed} fallidas`,
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

  container.querySelector('#goto-bursts-btn')?.addEventListener('click', () => navigate('bursts'));
  container
    .querySelector('#goto-tournament-btn')
    ?.addEventListener('click', () => navigate('tournament'));
}
