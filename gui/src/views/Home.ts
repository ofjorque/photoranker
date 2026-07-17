// Pantalla de inicio: elegir/abrir carpeta, correr init/prune/burst-detect,
// y los controles de deshacer/reiniciar agregados por feedback de uso real
// (ver docs/fase3-torneo.md, "Deshacer / reiniciar"). Envuelve exactamente
// los comandos del CLI, sin lógica nueva.
import { invoke } from '@tauri-apps/api/core';
import { cli, CliError, extractLogStatus } from '../api';
import { getProject, setProject, dbPathFor } from '../state';
import { showToast } from '../toast';
import { navigate } from '../router';
import { showLoadingOverlay, withButtonBusy } from '../components/LoadingOverlay';
import { confirmDialog } from '../components/ConfirmDialog';
import { t } from '../i18n';

function initPhases(): string[] {
  return [
    t('home.init.phase.scanning'),
    t('home.init.phase.thumbnails'),
    t('home.init.phase.phash'),
    t('home.init.phase.quality'),
    t('home.init.phase.pairing'),
  ];
}

export async function renderHome(container: HTMLElement): Promise<void> {
  const project = getProject();

  container.innerHTML = `
    <div class="view">
      <h1>${t('home.title')}</h1>
      <div class="panel">
        <div class="field">
          <label>${t('home.folder.label')}</label>
          <div class="input-row">
            <input type="text" id="folder-input" placeholder="${t('home.folder.placeholder')}" value="${
              project?.folderPath ?? ''
            }" style="flex:1" />
            <button class="btn" id="pick-folder-btn">${t('home.folder.pick')}</button>
          </div>
        </div>
        <div class="spacer-md"></div>
        <button class="btn btn-primary" id="init-btn">${t('home.init.button')}</button>
      </div>

      ${
        project
          ? `
      <div class="panel">
        <h2>${t('home.actions.heading', { folderPath: project.folderPath })}</h2>
        <div class="panel-row panel-row--wrap">
          <button class="btn" id="prune-btn">prune</button>
          <button class="btn" id="burst-detect-btn">burst-detect</button>
          <button class="btn btn-primary" id="goto-bursts-btn">${t('home.actions.gotoBursts')}</button>
          <button class="btn btn-primary" id="goto-tournament-btn">${t('home.actions.gotoTournament')}</button>
        </div>
      </div>
      <div class="panel" id="result-panel" style="display:none"></div>

      <div class="panel panel-danger-zone">
        <h2>${t('home.dangerZone.title')}</h2>
        <p>${t('home.dangerZone.description')}</p>
        <div class="panel-row panel-row--wrap">
          <button class="btn" id="undo-btn">${t('home.dangerZone.undo')}</button>
          <button class="btn btn-danger" id="reset-tournament-btn">${t('home.dangerZone.reset')}</button>
        </div>
      </div>
      `
          : ''
      }

      <div class="panel panel-danger-zone">
        <h2>${t('home.globalIndex.title')}</h2>
        <p>${t('home.globalIndex.description')}</p>
        <div class="panel-row panel-row--wrap">
          ${
            project
              ? `<button class="btn" id="resync-global-btn" title="${t('home.globalIndex.resyncTitle')}">resync-global</button>`
              : ''
          }
          <button class="btn btn-danger" id="reset-global-btn" title="${t('home.globalIndex.resetTitle')}">reset-global-index</button>
        </div>
      </div>
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
      showToast(t('home.folder.needFolder'), true);
      return;
    }
    const run = cli.initAsync(folder, (_stream, line) => {
      const status = extractLogStatus(line);
      if (status) overlayHandle.setStatus(status);
    });
    const overlayHandle = showLoadingOverlay(t('home.init.loadingTitle'), initPhases(), {
      onCancel: () => run.cancel(),
    });
    try {
      const data = await run.promise;
      setProject({ folderPath: folder, dbPath: dbPathFor(folder) });
      const pairedNote =
        data.paired_raw_jpeg > 0 ? t('home.init.pairedNote', { count: data.paired_raw_jpeg }) : '';
      showToast(
        t('home.init.result', {
          ok: data.inserted_ok,
          existing: data.skipped_existing,
          failed: data.inserted_failed,
          pairedNote,
        }),
      );
      renderHome(container);
    } catch (e) {
      if (e instanceof CliError && e.code === 'CANCELLED') {
        showToast(t('home.init.cancelled'));
      } else {
        showToast(e instanceof CliError ? e.message : String(e), true);
      }
    } finally {
      overlayHandle.close();
    }
  });

  container.querySelector<HTMLButtonElement>('#prune-btn')?.addEventListener('click', async (e) => {
    if (!project) return;
    const btn = e.currentTarget as HTMLButtonElement;
    try {
      const data = await withButtonBusy(btn, t('home.busyRunning'), () => cli.prune(project.dbPath));
      showResult('prune', data);
      showToast(t('home.prune.result', { count: data.marked_missing }));
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector<HTMLButtonElement>('#burst-detect-btn')?.addEventListener('click', async (e) => {
    if (!project) return;
    const btn = e.currentTarget as HTMLButtonElement;
    try {
      const data = await withButtonBusy(btn, t('home.busyRunning'), () => cli.burstDetect(project.dbPath));
      showResult('burst-detect', data);
      showToast(t('home.burstDetect.result', { count: data.bursts_created }));
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector<HTMLButtonElement>('#undo-btn')?.addEventListener('click', async (e) => {
    if (!project) return;
    const btn = e.currentTarget as HTMLButtonElement;
    try {
      const data = await withButtonBusy(btn, t('home.undo.busy'), () => cli.tournamentUndo(project.dbPath));
      showResult('tournament-undo', data);
      showToast(
        t('home.undo.result', { groupId: data.group_id.slice(0, 8), count: data.reverted_images.length }),
      );
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector<HTMLButtonElement>('#reset-tournament-btn')?.addEventListener('click', async (e) => {
    if (!project) return;
    // `e.currentTarget` deja de ser válido apenas termina la fase síncrona
    // del evento (el navegador lo pone en null) — hay que capturarlo ANTES
    // del primer `await`, no después (bug real: el `confirmDialog` de abajo
    // es asíncrono, así que leerlo después de esperarlo devolvía null y
    // `withButtonBusy` fallaba con "Cannot read properties of null (reading
    // 'textContent')").
    const btn = e.currentTarget as HTMLButtonElement;
    const confirmed = await confirmDialog({
      title: t('home.resetTournament.confirmTitle'),
      message: t('home.resetTournament.confirmMessage', { folderPath: project.folderPath }),
      confirmLabel: t('home.resetTournament.confirmLabel'),
      danger: true,
    });
    if (!confirmed) return;
    try {
      const data = await withButtonBusy(btn, t('home.resetTournament.busy'), () =>
        cli.tournamentReset(project.dbPath),
      );
      showResult('tournament-reset', data);
      showToast(t('home.resetTournament.result', { count: data.images_reset }));
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector<HTMLButtonElement>('#reset-global-btn')?.addEventListener('click', async (e) => {
    const btn = e.currentTarget as HTMLButtonElement;
    const confirmed = await confirmDialog({
      title: t('home.resetGlobal.confirmTitle'),
      message: t('home.resetGlobal.confirmMessage'),
      confirmLabel: t('home.resetGlobal.confirmLabel'),
      danger: true,
    });
    if (!confirmed) return;
    try {
      const data = await withButtonBusy(btn, t('home.resetGlobal.busy'), () => cli.resetGlobalIndex());
      showResult('reset-global-index', data);
      showToast(t('home.resetGlobal.result', { count: data.rows_deleted }));
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector<HTMLButtonElement>('#resync-global-btn')?.addEventListener('click', async (e) => {
    if (!project) return;
    const btn = e.currentTarget as HTMLButtonElement;
    try {
      const data = await withButtonBusy(btn, t('home.resyncGlobal.busy'), () => cli.resyncGlobal(project.folderPath));
      showResult('resync-global', data);
      showToast(t('home.resyncGlobal.result', { count: data.rows_updated }));
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector('#goto-bursts-btn')?.addEventListener('click', () => navigate('bursts'));
  container
    .querySelector('#goto-tournament-btn')
    ?.addEventListener('click', () => navigate('tournament'));
}
