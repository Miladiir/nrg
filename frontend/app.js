// Generation and validation run locally through the id-core WebAssembly build
// (`./pkg/id_core.js`) — the exact same Rust code the HTTP API executes.
// Only lookups call the API: LEI needs the server-side GLEIF cache and rate
// limit, and EIC/OBIS share the same code path for consistency.
import initWasm, { generate as wasmGenerate, validate as wasmValidate } from "./pkg/id_core.js";

// Static identifier list; every entry maps 1:1 to /api/v1/{slug}/{action}
// routes and to the WebAssembly `generate`/`validate` slugs.
const GROUPS = [
  {
    name: "Energie · Lokationen",
    items: [
      { slug: "malo", label: "Marktlokations-ID (MaLo-ID)", generate: true, validate: true },
      { slug: "melo", label: "Messlokations-ID (MeLo-ID)", generate: true, validate: true },
      { slug: "nelo", label: "Netzlokations-ID (NeLo-ID)", generate: true, validate: true },
      { slug: "nebe", label: "Netzbereichs-ID (NeBe-ID)", generate: true, validate: true },
    ],
  },
  {
    name: "Energie · Marktpartner & Register",
    items: [
      { slug: "mp-id", label: "BDEW-/DVGW-Marktpartner-ID", generate: true, validate: true, sector: true },
      { slug: "mastr", label: "MaStR-Nummer", generate: true, validate: true, sector: true, mastr: true },
      { slug: "eic", label: "Energy Identification Code (EIC)", validate: true, lookup: true },
    ],
  },
  {
    name: "Energie · Ressourcen",
    items: [
      { slug: "cr-id", label: "Cluster-Ressource (CR-ID)", generate: true },
      { slug: "sg-id", label: "Steuergruppe (SG-ID)", generate: true },
      { slug: "sr-id", label: "Steuerbare Ressource (SR-ID)", generate: true },
      { slug: "tr-id", label: "Technische Ressource (TR-ID)", generate: true },
      { slug: "package-id", label: "Paket-ID", generate: true },
    ],
  },
  {
    name: "Zahlungsverkehr",
    items: [
      { slug: "iban", label: "IBAN", generate: true, validate: true, country: true, format: true },
      { slug: "bic", label: "BIC", generate: true, validate: true, bic: true },
      { slug: "creditor-id", label: "Gläubiger-ID (Deutschland)", generate: true, validate: true },
      { slug: "mandate-reference", label: "Mandatsreferenz", generate: true, validate: true },
      { slug: "end-to-end-id", label: "End-to-End-ID", generate: true, validate: true },
      { slug: "rf-reference", label: "RF-Referenz (ISO 11649)", generate: true, validate: true, invoice: true, format: true },
      { slug: "uetr", label: "UETR", generate: true, validate: true },
    ],
  },
  {
    name: "Messwesen",
    items: [
      { slug: "obis", label: "OBIS-Kennzahl", validate: true, lookup: true },
      { slug: "din-43849", label: "DIN-43849-Gerätekennung", validate: true },
    ],
  },
  {
    name: "Unternehmen",
    items: [
      { slug: "vat-id", label: "USt-IdNr. (Deutschland)", validate: true },
      { slug: "lei", label: "LEI", validate: true, lookup: true },
    ],
  },
];

const IDENTIFIERS = GROUPS.flatMap((group) => group.items);

const elements = {};
let toastTimer;

document.addEventListener("DOMContentLoaded", async () => {
  elements.navigation = document.getElementById("navigation");
  elements.detail = document.getElementById("detail");
  elements.toast = document.getElementById("toast");
  try {
    await initWasm();
  } catch (error) {
    elements.detail.replaceChildren(
      notice(`Das WebAssembly-Modul konnte nicht geladen werden: ${error instanceof Error ? error.message : String(error)}`, "error"),
    );
    return;
  }
  renderNavigation();
  window.addEventListener("hashchange", renderDetail);
  renderDetail();
});

function selectedIdentifier() {
  const slug = decodeURIComponent(location.hash.replace(/^#/, ""));
  return IDENTIFIERS.find((item) => item.slug === slug) ?? IDENTIFIERS[0];
}

function renderNavigation() {
  elements.navigation.replaceChildren();
  for (const group of GROUPS) {
    const section = element("section", "nav-group");
    section.append(element("h3", null, group.name));
    for (const item of group.items) {
      const button = element("button", "nav-item", item.label);
      button.type = "button";
      button.dataset.slug = item.slug;
      button.addEventListener("click", () => {
        location.hash = `#${encodeURIComponent(item.slug)}`;
      });
      section.append(button);
    }
    elements.navigation.append(section);
  }
}

function renderDetail() {
  const identifier = selectedIdentifier();
  document.querySelectorAll(".nav-item").forEach((button) => {
    button.classList.toggle("is-current", button.dataset.slug === identifier.slug);
  });

  const wrapper = element("div");
  const heading = element("div", "detail-heading");
  heading.append(element("h1", null, identifier.label));
  wrapper.append(heading);

  const layout = element("div", "action-layout");
  if (identifier.generate) layout.append(generateCard(identifier));
  if (identifier.validate) layout.append(validateCard(identifier));
  if (identifier.lookup) layout.append(lookupCard(identifier));
  wrapper.append(layout);

  elements.detail.replaceChildren(wrapper);
  window.scrollTo({ top: 0, behavior: "auto" });
}

function generateCard(identifier) {
  const card = element("section", "panel-card");
  const form = element("form");
  form.append(element("h2", null, "Generieren"));
  const fields = element("div", "form-grid");

  const count = inputField("Anzahl", "number", "1");
  count.input.min = "1";
  count.input.max = "100";
  count.input.required = true;
  const seed = inputField("Seed (optional)", "text", "");
  seed.input.placeholder = "Leer = zufälliger Seed";
  seed.input.autocomplete = "off";
  seed.input.spellcheck = false;
  fields.append(count.wrapper, seed.wrapper);

  let sector;
  if (identifier.sector) {
    sector = selectField("Sparte", [
      { value: "electricity", label: "Strom" },
      { value: "gas", label: "Gas" },
    ]);
    fields.append(sector.wrapper);
  }

  let format;
  if (identifier.format) {
    format = selectField("Darstellung", [
      { value: "electronic", label: "Elektronisch" },
      { value: "formatted", label: "Formatiert" },
    ]);
    fields.append(format.wrapper);
  }

  let country;
  if (identifier.country) {
    country = inputField("Land (ISO Alpha-2)", "text", "DE");
    country.input.maxLength = 2;
    country.input.pattern = "[A-Za-z]{2}";
    country.input.autocapitalize = "characters";
    country.input.addEventListener("input", () => {
      country.input.value = country.input.value.toUpperCase();
    });
    fields.append(country.wrapper);
  }

  let includeBranch;
  if (identifier.bic) {
    includeBranch = selectField("Form", [
      { value: "false", label: "8-stelliger BIC" },
      { value: "true", label: "11-stelliger BIC" },
    ]);
    fields.append(includeBranch.wrapper);
  }

  let invoiceReference;
  if (identifier.invoice) {
    invoiceReference = inputField("Rechnungsreferenz (optional)", "text", "");
    invoiceReference.input.maxLength = 21;
    fields.append(invoiceReference.wrapper);
  }

  let mastrPrefix;
  let mastrRoleSuffix;
  if (identifier.mastr) {
    mastrPrefix = inputField("Präfix (optional)", "text", "");
    mastrPrefix.input.maxLength = 3;
    mastrRoleSuffix = inputField("Rollen-Suffix (optional)", "text", "");
    mastrRoleSuffix.input.maxLength = 2;
    fields.append(mastrPrefix.wrapper, mastrRoleSuffix.wrapper);
  }

  form.append(fields);
  const submit = element("button", "primary-button", "Werte erzeugen");
  submit.type = "submit";
  form.append(element("div", "button-row", submit));
  const resultHost = element("div", "result-area");
  card.append(form, resultHost);

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const options = compactObject({
      count: Number(count.input.value),
      seed: seed.input.value.trim() || undefined,
      sector: sector?.input.value || undefined,
      format: format?.input.value || undefined,
      country: country?.input.value.trim().toUpperCase() || undefined,
      include_branch: includeBranch ? includeBranch.input.value === "true" : undefined,
      invoice_reference: invoiceReference?.input.value.trim() || undefined,
      prefix: mastrPrefix?.input.value.trim().toUpperCase() || undefined,
      role_suffix: mastrRoleSuffix?.input.value.trim().toUpperCase() || undefined,
    });
    submitWasm(() => wasmGenerate(identifier.slug, JSON.stringify(options)), resultHost, renderGenerateResult);
  });
  return card;
}

function validateCard(identifier) {
  const card = element("section", "panel-card");
  const form = element("form");
  form.append(element("h2", null, "Validieren"));
  const value = inputField("Kennung", "text", "");
  value.input.required = true;
  value.input.autocomplete = "off";
  value.input.spellcheck = false;
  form.append(value.wrapper);
  const submit = element("button", "primary-button", "Prüfen");
  submit.type = "submit";
  form.append(element("div", "button-row", submit));
  const resultHost = element("div", "result-area");
  card.append(form, resultHost);

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    submitWasm(() => wasmValidate(identifier.slug, value.input.value.trim()), resultHost, renderValidateResult);
  });
  return card;
}

function lookupCard(identifier) {
  const card = element("section", "panel-card");
  const form = element("form");
  form.append(element("h2", null, "Nachschlagen"));
  const value = inputField("Kennung", "text", "");
  value.input.required = true;
  value.input.autocomplete = "off";
  value.input.spellcheck = false;
  form.append(value.wrapper);
  const submit = element("button", "primary-button", "Nachschlagen");
  submit.type = "submit";
  form.append(element("div", "button-row", submit));
  const resultHost = element("div", "result-area");
  card.append(form, resultHost);

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    await submitRequest(
      `/api/v1/${identifier.slug}/lookup`,
      { id: value.input.value.trim() },
      submit,
      resultHost,
      renderLookupResult,
    );
  });
  return card;
}

function submitWasm(call, resultHost, renderer) {
  try {
    const result = JSON.parse(call());
    // A bare {"error": ...} (no "valid" field) is a failed request; an invalid
    // validation result is rendered normally.
    if (result?.error && !("valid" in result)) throw new Error(result.error);
    resultHost.replaceChildren();
    renderer(result, resultHost);
  } catch (error) {
    resultHost.replaceChildren(notice(error instanceof Error ? error.message : String(error), "error"));
  }
}

async function submitRequest(path, body, submit, resultHost, renderer) {
  submit.disabled = true;
  submit.setAttribute("aria-busy", "true");
  resultHost.replaceChildren(notice("Anfrage läuft …"));
  try {
    const response = await fetch(path, {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(body),
    });
    const result = await response.json();
    if (!response.ok) throw new Error(result?.error ?? `HTTP ${response.status}`);
    resultHost.replaceChildren();
    renderer(result, resultHost);
  } catch (error) {
    resultHost.replaceChildren(notice(error instanceof Error ? error.message : String(error), "error"));
  } finally {
    submit.disabled = false;
    submit.removeAttribute("aria-busy");
  }
}

function renderGenerateResult(result, host) {
  const values = Array.isArray(result?.values) ? result.values : [];
  const toolbar = element("div", "result-toolbar");
  toolbar.append(element("h3", null, `${values.length} Wert${values.length === 1 ? "" : "e"}`));
  const copies = element("div", "copy-group");
  copies.append(copyButton("Kopieren", () => values.join("\n")));
  toolbar.append(copies);
  host.append(toolbar);

  const list = element("div", "result-list");
  for (const value of values) {
    const card = element("article", "result-card");
    card.append(element("p", "result-value", value));
    list.append(card);
  }
  host.append(list);
}

function renderValidateResult(result, host) {
  if (result?.valid === true) {
    host.append(notice("Kennung ist gültig.", "success"));
  } else {
    host.append(notice(`Kennung ist ungültig: ${result?.error ?? "unbekannter Fehler"}`, "error"));
  }
}

function renderLookupResult(result, host) {
  const pre = element("pre", "request-preview json-result");
  pre.textContent = JSON.stringify(result, null, 2);
  host.append(pre);
}

function copyButton(label, valueFactory) {
  const button = element("button", "copy-button", label);
  button.type = "button";
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(valueFactory());
      showToast("In die Zwischenablage kopiert.");
    } catch {
      showToast("Kopieren ist in diesem Browser nicht verfügbar.");
    }
  });
  return button;
}

function inputField(label, type, value) {
  const wrapper = element("label", "field");
  wrapper.append(element("span", null, label));
  const input = document.createElement("input");
  input.type = type;
  input.value = value;
  wrapper.append(input);
  return { wrapper, input };
}

function selectField(label, options) {
  const wrapper = element("label", "field");
  wrapper.append(element("span", null, label));
  const input = document.createElement("select");
  for (const option of options) {
    const node = document.createElement("option");
    node.value = option.value;
    node.textContent = option.label;
    input.append(node);
  }
  wrapper.append(input);
  return { wrapper, input };
}

function notice(message, kind = "") {
  return element("div", `inline-notice${kind ? ` is-${kind}` : ""}`, message);
}

function compactObject(value) {
  return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined && item !== ""));
}

function element(tagName, className, ...children) {
  const node = document.createElement(tagName);
  if (className) node.className = className;
  for (const child of children) {
    if (child == null) continue;
    node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return node;
}

function showToast(message) {
  clearTimeout(toastTimer);
  elements.toast.textContent = message;
  elements.toast.hidden = false;
  toastTimer = setTimeout(() => {
    elements.toast.hidden = true;
  }, 2400);
}
