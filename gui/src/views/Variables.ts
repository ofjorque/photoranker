// Variables personalizadas — ver docs/fase2-clustering.md / fase1-ingesta.md.
// `variable-tag` es un modo TUI que toma la terminal directamente
// (crossterm/ratatui) y no puede pilotearse como subproceso JSON — ver
// docs/fase5-gui.md. Replicar su recorrido "una imagen a la vez, salta con
// espacio" requeriría un comando nuevo que liste qué imágenes activas ya
// tienen valor para una variable dada, que el CLI no expone todavía; agregar
// ese comando es una decisión de alcance que excede esta pasada (ver
// CLAUDE.md, "no tomes decisiones de arquitectura por tu cuenta"). Por eso
// esta pantalla solo envuelve `variable-create`/`variable-list`/`variable-set`
// tal como existen, con asignación por lote id:valor.
import { cli, CliError } from '../api';
import type { UserVariable } from '../api/types';
import { getProject } from '../state';
import { showToast } from '../toast';

export async function renderVariables(container: HTMLElement): Promise<void> {
  const project = getProject();
  if (!project) {
    container.innerHTML =
      '<div class="view"><div class="empty-state">Abrí un proyecto primero.</div></div>';
    return;
  }
  const dbPath = project.dbPath;

  container.innerHTML = `
    <div class="view">
      <h1>Variables personalizadas</h1>

      <div class="panel">
        <h2>Crear variable</h2>
        <div style="display:grid; grid-template-columns: 1fr 140px 100px 100px; gap:8px; align-items:end;">
          <div class="field"><label>Nombre</label><input type="text" id="var-name" placeholder="Grado de nostalgia" /></div>
          <div class="field">
            <label>Tipo</label>
            <select id="var-type"><option value="ordinal">ordinal</option><option value="nominal">nominal</option></select>
          </div>
          <div class="field"><label>Min</label><input type="number" id="var-min" /></div>
          <div class="field"><label>Max</label><input type="number" id="var-max" /></div>
        </div>
        <div class="field" style="margin-top:8px">
          <label>Categorías (solo nominal) — formato "Etiqueta:codigo,Etiqueta:codigo"</label>
          <input type="text" id="var-categories" placeholder="No:0,Sí:1" />
        </div>
        <button class="btn btn-primary" id="create-var-btn" style="margin-top:12px">variable-create</button>
      </div>

      <div class="panel">
        <h2>Variables definidas</h2>
        <div id="var-list"><p>Cargando…</p></div>
      </div>

      <div class="panel">
        <h2>Asignar valores por lote</h2>
        <div class="field">
          <label>Variable</label>
          <select id="var-set-select"></select>
        </div>
        <div class="field" style="margin-top:8px">
          <label>Valores (uno por línea, formato id:valor)</label>
          <textarea id="var-set-values" rows="5" style="font-family:var(--font-family); background:var(--color-bg); color:var(--color-text); border:1px solid var(--color-border); border-radius:var(--radius-md); padding:8px;" placeholder="42:4&#10;17:2&#10;58:5"></textarea>
        </div>
        <button class="btn btn-primary" id="set-values-btn" style="margin-top:12px">variable-set</button>
      </div>
    </div>
  `;

  const listContainer = container.querySelector<HTMLElement>('#var-list')!;
  const setSelect = container.querySelector<HTMLSelectElement>('#var-set-select')!;

  async function refreshList() {
    try {
      const variables = await cli.variableList(dbPath);
      renderList(variables);
    } catch (e) {
      listContainer.innerHTML = `<div class="empty-state">${
        e instanceof CliError ? e.message : String(e)
      }</div>`;
    }
  }

  function renderList(variables: UserVariable[]) {
    if (variables.length === 0) {
      listContainer.innerHTML = '<div class="empty-state">Todavía no hay variables definidas.</div>';
      setSelect.innerHTML = '';
      return;
    }
    listContainer.innerHTML = `
      <table>
        <thead><tr><th>Nombre</th><th>Tipo</th><th>Rango / Categorías</th></tr></thead>
        <tbody>
          ${variables
            .map(
              (v) => `<tr><td>${v.name}</td><td>${v.var_type}</td><td>${
                v.var_type === 'ordinal'
                  ? `${v.min_value ?? '?'} – ${v.max_value ?? '?'}`
                  : v.categories.map((c) => `${c.label}=${c.code}`).join(', ')
              }</td></tr>`,
            )
            .join('')}
        </tbody>
      </table>`;
    setSelect.innerHTML = variables.map((v) => `<option value="${v.name}">${v.name}</option>`).join('');
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
      showToast('El nombre es obligatorio', true);
      return;
    }
    try {
      await cli.variableCreate(dbPath, name, varType, {
        min: minStr === '' ? undefined : Number(minStr),
        max: maxStr === '' ? undefined : Number(maxStr),
        categories: categories === '' ? undefined : categories,
      });
      showToast(`Variable "${name}" creada`);
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
        showToast(`Línea inválida: "${line}" (formato esperado id:valor)`, true);
        return;
      }
      values.push([id, val]);
    }
    if (!variable || values.length === 0) {
      showToast('Elegí una variable y al menos un valor', true);
      return;
    }
    try {
      const result = await cli.variableSet(dbPath, variable, values);
      showToast(`${result.values_set} valores asignados a "${variable}"`);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  });
}
