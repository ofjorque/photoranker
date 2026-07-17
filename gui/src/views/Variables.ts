// Variables personalizadas — ver docs/fase2-clustering.md / fase1-ingesta.md.
// Incluye una clasificación visual foto por foto (ver docs/fase5-gui.md,
// agregado por feedback de uso real: "debo poder editar las fotos de
// acuerdo a las variables creadas"). No llama a `variable-tag` (modo TUI que
// toma la terminal directamente vía crossterm/ratatui y no puede pilotearse
// como subproceso JSON) — en cambio replica su mecánica (una imagen a la
// vez, número asigna y avanza, flechas navegan) sobre `get-variable-values` +
// `variable-set`, los mismos comandos de siempre.
import { cli, CliError } from '../api';
import type { UserVariable, VariableValueEntry } from '../api/types';
import { getProject } from '../state';
import { showToast } from '../toast';
import { getThumbnailDataUrl } from '../api/thumbnailCache';
import { makeZoomable } from '../components/Lightbox';
import { confirmDialog } from '../components/ConfirmDialog';
import { icons } from '../components/icons';
import { isTypingTarget } from '../utils/dom';
import { t } from '../i18n';

export async function renderVariables(container: HTMLElement): Promise<() => void> {
  const cleanupFns: Array<() => void> = [];
  const cleanup = () => cleanupFns.forEach((fn) => fn());

  const project = getProject();
  if (!project) {
    container.innerHTML = `<div class="view"><div class="empty-state">${t('common.openProjectFirst')}</div></div>`;
    return cleanup;
  }
  const dbPath = project.dbPath;

  container.innerHTML = `
    <div class="view">
      <h1>${t('variables.title')}</h1>

      <div class="panel">
        <h2>${t('variables.create.title')}</h2>
        <div style="display:grid; grid-template-columns: 1fr 140px 100px 100px; gap:8px; align-items:end;">
          <div class="field"><label>${t('variables.field.name')}</label><input type="text" id="var-name" placeholder="${t('variables.create.namePlaceholder')}" /></div>
          <div class="field">
            <label>${t('variables.field.type')}</label>
            <select id="var-type"><option value="ordinal">ordinal</option><option value="nominal">nominal</option></select>
          </div>
          <div class="field"><label>${t('variables.field.min')}</label><input type="number" id="var-min" /></div>
          <div class="field"><label>${t('variables.field.max')}</label><input type="number" id="var-max" /></div>
        </div>
        <div class="field" style="margin-top:8px">
          <label>${t('variables.create.categoriesLabel')}</label>
          <input type="text" id="var-categories" placeholder="${t('variables.create.categoriesPlaceholder')}" />
        </div>
        <button class="btn btn-primary" id="create-var-btn" style="margin-top:12px">variable-create</button>
      </div>

      <div class="panel">
        <h2>${t('variables.list.title')}</h2>
        <div id="var-list"><p>${t('common.loading')}</p></div>
      </div>

      <div class="panel">
        <h2>${t('variables.classify.title')}</h2>
        <p>${t('variables.classify.description')}</p>
        <div class="input-row" style="align-items:flex-end">
          <div class="field" style="flex:1">
            <label>${t('variables.field.variable')}</label>
            <select id="classify-select"></select>
          </div>
          <button class="btn btn-primary" id="start-classify-btn">${t('variables.classify.start')}</button>
        </div>
        <div id="classify-area" style="margin-top:16px"></div>
      </div>

      <div class="panel">
        <h2>${t('variables.batch.title')}</h2>
        <div class="field">
          <label>${t('variables.field.variable')}</label>
          <select id="var-set-select"></select>
        </div>
        <div class="field" style="margin-top:8px">
          <label>${t('variables.batch.valuesLabel')}</label>
          <textarea id="var-set-values" rows="5" placeholder="42:4&#10;17:2&#10;58:5"></textarea>
        </div>
        <button class="btn btn-primary" id="set-values-btn" style="margin-top:12px">variable-set</button>
      </div>
    </div>
  `;

  const listContainer = container.querySelector<HTMLElement>('#var-list')!;
  const setSelect = container.querySelector<HTMLSelectElement>('#var-set-select')!;
  const classifySelect = container.querySelector<HTMLSelectElement>('#classify-select')!;
  const classifyArea = container.querySelector<HTMLElement>('#classify-area')!;

  let currentVariables: UserVariable[] = [];

  async function refreshList() {
    try {
      currentVariables = await cli.variableList(dbPath);
      renderList();
    } catch (e) {
      listContainer.innerHTML = `<div class="empty-state">${
        e instanceof CliError ? e.message : String(e)
      }</div>`;
    }
  }

  function renderList() {
    if (currentVariables.length === 0) {
      listContainer.innerHTML = `<div class="empty-state">${t('variables.list.empty')}</div>`;
      setSelect.innerHTML = '';
      classifySelect.innerHTML = '';
      return;
    }
    listContainer.innerHTML = `
      <table>
        <thead><tr><th>${t('variables.field.name')}</th><th>${t('variables.field.type')}</th><th>${t('variables.list.colRange')}</th><th></th></tr></thead>
        <tbody>
          ${currentVariables
            .map(
              (v) => `<tr><td>${v.name}</td><td>${v.var_type}</td><td>${
                v.var_type === 'ordinal'
                  ? `${v.min_value ?? '?'} – ${v.max_value ?? '?'}`
                  : v.categories.map((c) => `${c.label}=${c.code}`).join(', ')
              }</td><td><button class="btn btn-danger delete-var-btn" data-variable-name="${
                v.name
              }" title="${t('variables.list.deleteTitle')}">${icons.trash}</button></td></tr>`,
            )
            .join('')}
        </tbody>
      </table>`;
    const options = currentVariables
      .map((v) => `<option value="${v.name}">${v.name}</option>`)
      .join('');
    setSelect.innerHTML = options;
    classifySelect.innerHTML = options;

    listContainer.querySelectorAll<HTMLButtonElement>('.delete-var-btn').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const name = btn.dataset.variableName!;
        const confirmed = await confirmDialog({
          title: t('variables.list.deleteDialogTitle'),
          message: t('variables.list.deleteDialogMessage', { name }),
          confirmLabel: t('variables.list.deleteDialogConfirm'),
          danger: true,
        });
        if (!confirmed) return;
        try {
          const result = await cli.variableDelete(dbPath, name);
          showToast(t('variables.list.deleted', { name, count: result.values_deleted }));
          await refreshList();
        } catch (e) {
          showToast(e instanceof CliError ? e.message : String(e), true);
        }
      });
    });
  }

  await refreshList();

  container.querySelector('#create-var-btn')?.addEventListener('click', async () => {
    const name = container.querySelector<HTMLInputElement>('#var-name')!.value.trim();
    const varType = container.querySelector<HTMLSelectElement>('#var-type')!.value as
      | 'ordinal'
      | 'nominal';
    const minStr = container.querySelector<HTMLInputElement>('#var-min')!.value;
    const maxStr = container.querySelector<HTMLInputElement>('#var-max')!.value;
    const categories = container.querySelector<HTMLInputElement>('#var-categories')!.value.trim();
    if (!name) {
      showToast(t('variables.create.nameRequired'), true);
      return;
    }
    try {
      await cli.variableCreate(dbPath, name, varType, {
        min: minStr === '' ? undefined : Number(minStr),
        max: maxStr === '' ? undefined : Number(maxStr),
        categories: categories === '' ? undefined : categories,
      });
      showToast(t('variables.create.created', { name }));
      await refreshList();
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector('#set-values-btn')?.addEventListener('click', async () => {
    const variable = setSelect.value;
    const raw = container.querySelector<HTMLTextAreaElement>('#var-set-values')!.value;
    const lines = raw
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l.length > 0);
    const values: Array<[number, number]> = [];
    for (const line of lines) {
      const [idStr, valStr] = line.split(':');
      const id = Number(idStr);
      const val = Number(valStr);
      if (!Number.isFinite(id) || !Number.isFinite(val)) {
        showToast(t('variables.batch.invalidLine', { line }), true);
        return;
      }
      values.push([id, val]);
    }
    if (!variable || values.length === 0) {
      showToast(t('variables.batch.needVariableAndValues'), true);
      return;
    }
    try {
      const result = await cli.variableSet(dbPath, variable, values);
      showToast(t('variables.batch.result', { count: result.values_set, variable }));
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });

  container.querySelector('#start-classify-btn')?.addEventListener('click', async () => {
    const variableName = classifySelect.value;
    const variable = currentVariables.find((v) => v.name === variableName);
    if (!variable) {
      showToast(t('variables.classify.needVariable'), true);
      return;
    }
    // Cada "Empezar" reemplaza el listener de teclado anterior, si había uno.
    cleanup();
    cleanupFns.length = 0;
    classifyArea.innerHTML = `<p>${t('common.loading')}</p>`;
    let entries: VariableValueEntry[];
    try {
      entries = await cli.getVariableValues(dbPath, variableName);
    } catch (e) {
      classifyArea.innerHTML = `<div class="empty-state">${
        e instanceof CliError ? e.message : String(e)
      }</div>`;
      return;
    }
    if (entries.length === 0) {
      classifyArea.innerHTML = `<div class="empty-state">${t('variables.classify.noActiveImages')}</div>`;
      return;
    }
    const destroy = mountClassifier(classifyArea, dbPath, variable, entries);
    cleanupFns.push(destroy);
  });

  return cleanup;
}

function mountClassifier(
  container: HTMLElement,
  dbPath: string,
  variable: UserVariable,
  entries: VariableValueEntry[],
): () => void {
  let index = 0;

  container.innerHTML = `
    <div class="panel" style="max-width:420px">
      <div class="panel-row">
        <span id="classify-counter" class="mono"></span>
        <span id="classify-current-value" class="badge badge-muted"></span>
      </div>
      <div class="thumb-wrap" style="aspect-ratio:4/3; margin:10px 0; display:flex; align-items:center; justify-content:center;">
        <div id="classify-thumb"></div>
      </div>
      <div id="classify-filename" style="font-size:12px; color:var(--color-text-muted); word-break:break-all;"></div>
      <div id="classify-options" style="display:flex; flex-wrap:wrap; gap:6px; margin-top:12px;"></div>
      <div class="ranking-hint" style="margin-top:12px;">
        ${t('variables.classifier.hint')}
      </div>
    </div>
  `;

  const counterEl = container.querySelector<HTMLElement>('#classify-counter')!;
  const currentValueEl = container.querySelector<HTMLElement>('#classify-current-value')!;
  const thumbEl = container.querySelector<HTMLElement>('#classify-thumb')!;
  const filenameEl = container.querySelector<HTMLElement>('#classify-filename')!;
  const optionsEl = container.querySelector<HTMLElement>('#classify-options')!;

  function optionLabel(code: number): string {
    if (variable.var_type === 'nominal') {
      const cat = variable.categories.find((c) => c.code === code);
      return cat ? `${code} = ${cat.label}` : String(code);
    }
    return String(code);
  }

  function validCodes(): number[] {
    if (variable.var_type === 'nominal') {
      return variable.categories.map((c) => c.code);
    }
    const min = variable.min_value ?? 1;
    const max = variable.max_value ?? 5;
    const codes: number[] = [];
    for (let v = min; v <= max; v++) codes.push(v);
    return codes;
  }

  async function render() {
    const entry = entries[index];
    counterEl.textContent = `${index + 1} / ${entries.length}`;
    currentValueEl.textContent =
      entry.value == null
        ? t('variables.classifier.unassigned')
        : t('variables.classifier.valueLabel', { label: optionLabel(entry.value) });
    filenameEl.textContent = entry.file_path.split(/[\\/]/).pop() ?? entry.file_path;
    thumbEl.innerHTML = t('common.loading');

    const codes = validCodes();
    optionsEl.innerHTML = '';
    for (const code of codes) {
      const btn = document.createElement('button');
      btn.className = 'btn' + (entry.value === code ? ' btn-primary' : '');
      btn.textContent = optionLabel(code);
      btn.addEventListener('click', () => assign(code));
      optionsEl.appendChild(btn);
    }

    const url = await getThumbnailDataUrl(dbPath, entry.id);
    thumbEl.innerHTML = '';
    if (url) {
      const img = document.createElement('img');
      img.src = url;
      img.style.width = '100%';
      img.style.height = '100%';
      img.style.objectFit = 'cover';
      thumbEl.appendChild(img);
      // makeZoomable se aplica a `img` (recreado en cada render), no a
      // `thumbEl` (persistente) — evita acumular listeners de click en cada
      // navegación (ver el mismo bug ya corregido en RankingBoard).
      makeZoomable(img, () => url, filenameEl.textContent ?? '');
    } else {
      thumbEl.textContent = t('common.noThumbnail');
    }
  }

  async function assign(code: number) {
    const entry = entries[index];
    try {
      await cli.variableSet(dbPath, variable.name, [[entry.id, code]]);
      entry.value = code;
      showToast(
        t('variables.classifier.assigned', {
          file: entry.file_path.split(/[\\/]/).pop() ?? entry.file_path,
          label: optionLabel(code),
        }),
      );
      goNext();
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  }

  function goNext() {
    if (index < entries.length - 1) {
      index += 1;
      render();
    } else {
      showToast(t('variables.classifier.lastImage'));
    }
  }

  function goPrev() {
    if (index > 0) {
      index -= 1;
      render();
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (isTypingTarget(e.target)) return;
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      goPrev();
    } else if (e.key === 'ArrowRight' || e.key === ' ') {
      e.preventDefault();
      goNext();
    } else if (e.key === 'Backspace') {
      e.preventDefault();
      goPrev();
    } else {
      const n = Number(e.key);
      if (Number.isInteger(n) && validCodes().includes(n)) {
        e.preventDefault();
        assign(n);
      }
    }
  }

  document.addEventListener('keydown', onKeyDown);
  render();

  return () => document.removeEventListener('keydown', onKeyDown);
}
