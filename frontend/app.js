const CATALOG_ENDPOINT = "/api/v1/catalog";
const SCENARIO_ENDPOINT = "/api/v1/scenarios";

const roleLabels = {
  market_partner: "Marktpartner",
  supplier: "Lieferant",
  grid_operator: "Netzbetreiber",
  metering_point_operator: "Messstellenbetreiber",
  balancing_responsible_party: "Bilanzkreisverantwortlicher",
  asset_operator: "Anlagenbetreiber",
};

const sectorLabels = {
  electricity: "Strom",
  gas: "Gas",
  cross_sector: "Spartenübergreifend",
};

const capabilityLabels = {
  generate: "Generieren",
  validate: "Validieren",
  parse: "Zerlegen",
  lookup: "Nachschlagen",
  negative: "Negativfixture",
  negative_fixture: "Negativfixture",
};

const profileLabels = {
  official_test_fixture: "Offizielles Testfixture",
  synthetic_non_routable: "Synthetisch, nicht routbar",
  directory_plausible: "Verzeichnisplausibel",
  checksum_only: "Nur Format und Prüfziffer",
  test_training_pattern: "Test-&-Training-Muster",
  syntax_only: "Nur Syntax",
  directory_value: "Verzeichniswert",
  official_example: "Offizielles Registerbeispiel",
};

const allocationLabels = {
  centrally_allocated: "Zentral vergeben",
  directory_backed: "Verzeichnisgestützt",
  issuer_assigned: "Vom Aussteller vergeben",
  self_assigned: "Selbst vergeben",
  not_applicable: "Nicht anwendbar",
};

const checksumLabels = {
  mod97: "MOD 97",
  bdew_lok_waggon: "Lok-Waggon",
  bdew_ascii: "BDEW ASCII",
  ean_mod10: "EAN Modulo 10",
  eic_check_character: "EIC-Prüfzeichen",
};

const checkLabels = {
  valid: "Gültig",
  invalid: "Ungültig",
  found: "Gefunden",
  not_found: "Nicht gefunden",
  not_checked: "Nicht geprüft",
  not_applicable: "Nicht anwendbar",
  none: "Keine Garantie",
  within_batch: "Im Batch eindeutig",
  unknown: "Unbekannt",
  true: "Ja",
  false: "Nein",
};

const state = {
  catalog: null,
  identifiers: [],
  selected: null,
  activeAction: null,
  role: "",
  sector: "",
  search: "",
  view: "overview",
  result: null,
  scenarios: null,
  selectedScenario: null,
};

const elements = {};
let toastTimer;

document.addEventListener("DOMContentLoaded", initialize);

async function initialize() {
  cacheElements();
  bindGlobalEvents();

  try {
    const response = await fetch(CATALOG_ENDPOINT, { headers: { Accept: "application/json" } });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const catalog = await response.json();
    if (!catalog || !Array.isArray(catalog.identifiers)) {
      throw new Error("Die Katalogantwort enthält keine Kennungsliste.");
    }

    state.catalog = catalog;
    state.identifiers = catalog.identifiers.filter(isCatalogIdentifier);
    elements.appStatus.hidden = true;
    elements.app.hidden = false;
    elements.catalogVersion.textContent = `Katalog ${catalog.catalog_version ?? "–"} · Generator ${catalog.generator_version ?? "–"}`;
    syncStateFromHash();
    renderNavigation();
    renderCurrentView();
  } catch (error) {
    elements.appStatus.classList.add("is-error");
    elements.appStatus.replaceChildren(
      textNode("Der Kennungskatalog konnte nicht geladen werden. "),
      textNode(error instanceof Error ? error.message : String(error)),
    );
  }
}

function cacheElements() {
  for (const id of [
    "appStatus", "app", "roleFilter", "sectorFilter", "searchFilter", "resetFilters",
    "catalogNavigation", "catalogCount", "catalogVersion", "overviewView", "detailView",
    "scenarioView", "catalogGrid", "emptyCatalog", "overviewLead", "detailHeader",
    "detailTabs", "detailPanel", "backToOverview", "scenarioStatus", "scenarioContent", "toast",
  ]) {
    elements[id] = document.getElementById(id);
  }
}

function bindGlobalEvents() {
  elements.roleFilter.addEventListener("change", () => {
    state.role = elements.roleFilter.value;
    renderNavigation();
    renderOverview();
  });
  elements.sectorFilter.addEventListener("change", () => {
    state.sector = elements.sectorFilter.value;
    renderNavigation();
    renderOverview();
  });
  elements.searchFilter.addEventListener("input", () => {
    state.search = elements.searchFilter.value.trim().toLocaleLowerCase("de");
    renderNavigation();
    renderOverview();
  });
  elements.resetFilters.addEventListener("click", resetFilters);
  elements.backToOverview.addEventListener("click", () => navigate("overview"));
  document.querySelectorAll("[data-view]").forEach((button) => {
    button.addEventListener("click", () => navigate(button.dataset.view));
  });
  window.addEventListener("hashchange", () => {
    syncStateFromHash();
    renderCurrentView();
  });
}

function isCatalogIdentifier(identifier) {
  return identifier
    && typeof identifier.slug === "string"
    && typeof identifier.label === "string"
    && Array.isArray(identifier.operations);
}

function resetFilters() {
  state.role = "";
  state.sector = "";
  state.search = "";
  elements.roleFilter.value = "";
  elements.sectorFilter.value = "";
  elements.searchFilter.value = "";
  renderNavigation();
  renderOverview();
}

function syncStateFromHash() {
  const hash = decodeURIComponent(location.hash.replace(/^#/, ""));
  if (hash === "scenarios") {
    state.view = "scenarios";
    state.selected = null;
    return;
  }
  if (hash.startsWith("identifier/")) {
    const slug = hash.slice("identifier/".length);
    const identifier = state.identifiers.find((item) => item.slug === slug);
    if (identifier) {
      state.view = "detail";
      state.selected = identifier;
      state.activeAction = preferredAction(identifier);
      return;
    }
  }
  state.view = "overview";
  state.selected = null;
}

function navigate(destination) {
  const hash = destination === "overview" ? "#overview" : destination === "scenarios" ? "#scenarios" : `#identifier/${encodeURIComponent(destination)}`;
  if (location.hash === hash) {
    syncStateFromHash();
    renderCurrentView();
  } else {
    location.hash = hash;
  }
}

function renderCurrentView() {
  const isOverview = state.view === "overview";
  const isDetail = state.view === "detail";
  const isScenarios = state.view === "scenarios";
  elements.overviewView.hidden = !isOverview;
  elements.detailView.hidden = !isDetail;
  elements.scenarioView.hidden = !isScenarios;
  updateCurrentNavigation();

  if (isOverview) renderOverview();
  if (isDetail && state.selected) renderDetail();
  if (isScenarios) renderScenarios();
  window.scrollTo({ top: 0, behavior: "auto" });
}

function filteredIdentifiers() {
  return state.identifiers.filter((identifier) => {
    const roleMatch = !state.role || identifier.roles?.includes(state.role);
    const sectorMatch = !state.sector
      || identifier.sectors?.includes(state.sector)
      || identifier.sectors?.includes("cross_sector");
    const haystack = [
      identifier.label,
      identifier.description,
      identifier.slug,
      identifier.kind,
      identifier.domain,
      primaryTag(identifier),
      ...(identifier.roles ?? []),
      ...(identifier.sectors ?? []),
      ...(identifier.capabilities ?? []),
      ...(identifier.examples ?? []).flatMap((example) => [example.value, example.label]),
      ...(identifier.sources ?? []).map((source) => source.label),
    ].join(" ").toLocaleLowerCase("de");
    return roleMatch && sectorMatch && (!state.search || haystack.includes(state.search));
  });
}

function renderNavigation() {
  const filtered = filteredIdentifiers();
  elements.catalogCount.textContent = String(filtered.length);
  const groups = groupBy(filtered, primaryTag);
  elements.catalogNavigation.replaceChildren();

  for (const [groupName, identifiers] of groups) {
    const section = element("section", "nav-group");
    section.append(element("h3", null, groupName));
    for (const identifier of identifiers.sort(sortByLabel)) {
      const button = element("button", "nav-item");
      button.type = "button";
      button.dataset.slug = identifier.slug;
      button.append(element("span", null, identifier.label), element("span", "nav-hint", identifier.slug));
      button.addEventListener("click", () => navigate(identifier.slug));
      section.append(button);
    }
    elements.catalogNavigation.append(section);
  }
  updateCurrentNavigation();
}

function updateCurrentNavigation() {
  document.querySelectorAll(".nav-overview").forEach((button) => {
    button.classList.toggle("is-current", button.dataset.view === state.view);
  });
  document.querySelectorAll(".nav-item").forEach((button) => {
    button.classList.toggle("is-current", state.view === "detail" && button.dataset.slug === state.selected?.slug);
  });
}

function renderOverview() {
  if (!state.catalog || state.view !== "overview") return;
  const identifiers = filteredIdentifiers().sort((a, b) => {
    const domain = primaryTag(a).localeCompare(primaryTag(b), "de");
    return domain || sortByLabel(a, b);
  });
  elements.overviewLead.textContent = activeFilterDescription(identifiers.length);
  elements.emptyCatalog.hidden = identifiers.length > 0;
  elements.catalogGrid.replaceChildren(...identifiers.map(identifierCard));
}

function identifierCard(identifier) {
  const card = element("article", "catalog-card");
  const header = element("div");
  header.append(
    element("div", "catalog-card__domain", primaryTag(identifier)),
    element("h2", null, identifier.label),
    element("p", "catalog-card__slug", identifier.slug),
    chipList((identifier.capabilities ?? []).map(labelCapability).slice(0, 4)),
  );
  const footer = element("div", "catalog-card__footer");
  const button = element("button", "card-button", "Öffnen");
  button.type = "button";
  button.setAttribute("aria-label", `${identifier.label} öffnen`);
  button.addEventListener("click", () => navigate(identifier.slug));
  footer.append(button);
  card.append(header, footer);
  return card;
}

function activeFilterDescription(count) {
  const qualifiers = [];
  if (state.role) qualifiers.push(roleLabels[state.role] ?? humanize(state.role));
  if (state.sector) qualifiers.push(sectorLabels[state.sector] ?? humanize(state.sector));
  if (state.search) qualifiers.push(`Suche „${state.search}“`);
  if (!qualifiers.length) return `${count} Kennungen – direkt aus dem NRG-Katalog.`;
  return `${count} Treffer für ${qualifiers.join(" · ")}. Spartenübergreifende Kennungen werden bei Strom und Gas berücksichtigt.`;
}

function renderDetail() {
  const identifier = state.selected;
  if (!identifier) return;
  state.result = null;
  renderDetailHeader(identifier);
  renderActionTabs(identifier);
  renderActionPanel(identifier, state.activeAction);
  requestAnimationFrame(() => document.getElementById("detailTitle")?.focus({ preventScroll: true }));
}

function renderDetailHeader(identifier) {
  const wrapper = element("div", "detail-heading");
  const eyebrow = element("p", "eyebrow", primaryTag(identifier));
  const title = element("h1", null, identifier.label);
  title.id = "detailTitle";
  title.tabIndex = -1;
  wrapper.append(eyebrow, title, element("p", "detail-lead", useDescription(identifier)));

  const facets = [
    ...(identifier.roles ?? []).map((role) => roleLabels[role] ?? humanize(role)),
    ...(identifier.sectors ?? []).map((sector) => sectorLabels[sector] ?? humanize(sector)),
  ];
  wrapper.append(chipList(facets, true));

  const metadata = element("dl", "metadata-grid");
  metadata.append(
    metadataItem("Vergabemodell", allocationLabels[identifier.allocation_model] ?? humanize(identifier.allocation_model)),
    metadataItem("Prüfziffer", identifier.checksum_scheme ? (checksumLabels[identifier.checksum_scheme] ?? humanize(identifier.checksum_scheme)) : "Keine"),
    metadataItem("Profile", (identifier.generation_profiles ?? []).map(labelProfile).join(", ") || "Keine Erzeugung"),
    metadataItem("Katalogschlüssel", identifier.slug),
  );
  wrapper.append(metadata, detailGuidance(identifier));
  elements.detailHeader.replaceChildren(wrapper);
}

function detailGuidance(identifier) {
  const grid = element("div", "detail-guide-grid");

  const formatCard = element("section", "detail-guide-card");
  formatCard.append(
    element("h2", null, "Format und Prüfziffer"),
    element(
      "p",
      null,
      identifier.format_description
        || `Die Struktur wird typbezogen geprüft. ${identifier.checksum_scheme ? `Prüfverfahren: ${checksumLabels[identifier.checksum_scheme] ?? humanize(identifier.checksum_scheme)}.` : "Für diese Kennung ist keine standardisierte Prüfziffer hinterlegt."}`,
    ),
  );

  const sourceCard = element("section", "detail-guide-card");
  sourceCard.append(element("h2", null, "Beispiele und Quellen"));
  const examples = Array.isArray(identifier.examples) ? identifier.examples : [];
  if (examples.length) {
    const exampleList = element("ul", "reference-list");
    for (const example of examples) {
      const item = element("li");
      item.append(
        element("code", null, example.value ?? String(example)),
        example.label ? document.createTextNode(` – ${example.label}`) : document.createTextNode(""),
      );
      exampleList.append(item);
    }
    sourceCard.append(exampleList);
  } else {
    sourceCard.append(element("p", "help-text", "Erzeugte Ergebnisse dienen als typbezogene Beispiele."));
  }

  const sources = Array.isArray(identifier.sources) ? identifier.sources : [];
  if (sources.length) {
    const sourceList = element("ul", "reference-list");
    for (const source of sources) {
      const item = element("li");
      const link = element("a", null, source.label ?? source.url);
      link.href = source.url;
      link.target = "_blank";
      link.rel = "noopener noreferrer";
      item.append(link);
      sourceList.append(item);
    }
    sourceCard.append(sourceList);
  }

  grid.append(formatCard, sourceCard);
  return grid;
}

function renderActionTabs(identifier) {
  const actions = availableActions(identifier);
  if (!actions.some((action) => action.key === state.activeAction)) state.activeAction = actions[0]?.key ?? null;
  elements.detailTabs.replaceChildren();
  for (const action of actions) {
    const button = element("button", "action-tab", action.label);
    button.type = "button";
    button.role = "tab";
    button.id = `tab-${action.key}`;
    button.setAttribute("aria-selected", String(action.key === state.activeAction));
    button.setAttribute("aria-controls", "detailPanel");
    button.addEventListener("click", () => {
      state.activeAction = action.key;
      state.result = null;
      renderActionTabs(identifier);
      renderActionPanel(identifier, action.key);
    });
    elements.detailTabs.append(button);
  }
}

function availableActions(identifier) {
  const actions = [];
  if (operationFor(identifier, "generate")) actions.push({ key: "generate", label: "Generieren" });
  if (operationFor(identifier, "validate")) actions.push({ key: "validate", label: "Validieren" });
  if (identifier.capabilities?.includes("parse") && (operationFor(identifier, "parse") || operationFor(identifier, "validate"))) {
    actions.push({ key: "parse", label: "Zerlegen" });
  }
  if (negativeOperation(identifier)) actions.push({ key: "negative", label: "Negativfixture" });
  if (operationFor(identifier, "lookup")) actions.push({ key: "lookup", label: "Nachschlagen" });
  return actions;
}

function preferredAction(identifier) {
  return availableActions(identifier)[0]?.key ?? null;
}

function renderActionPanel(identifier, action) {
  elements.detailPanel.replaceChildren();
  elements.detailPanel.setAttribute("role", "tabpanel");
  elements.detailPanel.setAttribute("aria-labelledby", `tab-${action}`);
  if (action === "generate") renderGeneratePanel(identifier);
  else if (action === "validate" || action === "parse") renderValidationPanel(identifier, action);
  else if (action === "negative") renderNegativePanel(identifier);
  else if (action === "lookup") renderLookupPanel(identifier);
  else elements.detailPanel.append(notice("Für diese Kennung ist keine öffentliche Aktion hinterlegt.", "error"));
}

function renderGeneratePanel(identifier) {
  const operation = operationFor(identifier, "generate");
  const layout = element("div", "action-layout");
  const formCard = element("section", "panel-card");
  const form = element("form");
  form.append(element("h2", null, "Testwerte generieren"));
  const fields = element("div", "form-grid");

  const profile = selectField("Profil", "generationProfile", (identifier.generation_profiles ?? []).map((value) => ({ value, label: labelProfile(value) })));
  profile.input.value = identifier.default_profile ?? identifier.generation_profiles?.[0] ?? "";
  const count = inputField("Anzahl", "generationCount", "number", "1");
  count.input.min = "1";
  count.input.max = "100";
  count.input.required = true;
  const format = selectField("Darstellung", "generationFormat", [
    { value: "electronic", label: "Elektronisch" },
    { value: "formatted", label: "Formatiert" },
  ]);
  const seed = inputField("Fixture-Seed", "generationSeed", "text", "integration-test-4711");
  seed.wrapper.classList.add("is-wide");
  fields.append(profile.wrapper, count.wrapper, format.wrapper, seed.wrapper);

  let sector;
  if (identifier.sectors?.includes("electricity") && identifier.sectors?.includes("gas")) {
    sector = selectField("Sparte für die Erzeugung", "generationSector", [
      { value: "electricity", label: "Strom" },
      { value: "gas", label: "Gas" },
    ]);
    fields.append(sector.wrapper);
  }

  let country;
  if (identifier.slug === "iban") {
    country = inputField("IBAN-Land (ISO Alpha-2)", "generationCountry", "text", "DE");
    country.input.maxLength = 2;
    country.input.pattern = "[A-Za-z]{2}";
    country.input.autocapitalize = "characters";
    country.input.setAttribute("aria-describedby", "generationCountryHelp");
    const countryHelp = element(
      "span",
      "help-text",
      "Außerhalb Deutschlands sind nur „Nur Format und Prüfziffer“ und offizielle Registerbeispiele verfügbar.",
    );
    countryHelp.id = "generationCountryHelp";
    country.wrapper.append(countryHelp);
    country.input.addEventListener("input", () => {
      country.input.value = country.input.value.toUpperCase();
      syncInternationalIbanProfiles(country.input, profile.input);
    });
    profile.input.addEventListener("change", () => syncInternationalIbanProfiles(country.input, profile.input));
    syncInternationalIbanProfiles(country.input, profile.input);
    fields.append(country.wrapper);
  }

  let includeBranch;
  if (identifier.slug === "bic") {
    includeBranch = selectField("Filialkennung", "generationBicBranch", [
      { value: "false", label: "8-stelliger BIC" },
      { value: "true", label: "11-stelliger BIC" },
    ]);
    fields.append(includeBranch.wrapper);
  }

  let invoiceReference;
  if (identifier.slug === "rf-reference") {
    invoiceReference = inputField("Rechnungsreferenz (optional)", "generationInvoiceReference", "text", "");
    invoiceReference.input.maxLength = 21;
    invoiceReference.input.setAttribute("aria-describedby", "generationInvoiceReferenceHelp");
    const invoiceHelp = element(
      "span",
      "help-text",
      "Eine vorgegebene Referenz erzeugt genau einen RF-Wert; die Anzahl wird dann automatisch auf 1 gesetzt.",
    );
    invoiceHelp.id = "generationInvoiceReferenceHelp";
    invoiceReference.wrapper.append(invoiceHelp);
    invoiceReference.input.addEventListener("input", () => {
      const hasReference = invoiceReference.input.value.trim().length > 0;
      if (hasReference && !count.input.disabled) {
        count.input.dataset.previousValue = count.input.value;
        count.input.value = "1";
        count.input.disabled = true;
      } else if (!hasReference && count.input.disabled) {
        count.input.disabled = false;
        count.input.value = count.input.dataset.previousValue || "1";
      }
    });
    fields.append(invoiceReference.wrapper);
  }

  let mastrPrefix;
  let mastrRoleSuffix;
  if (identifier.slug === "mastr") {
    mastrPrefix = inputField("MaStR-Präfix (optional)", "generationMastrPrefix", "text", "");
    mastrPrefix.input.maxLength = 3;
    mastrPrefix.input.pattern = "[A-Za-z]{3}";
    mastrPrefix.input.autocapitalize = "characters";
    mastrRoleSuffix = inputField("Rollen-Suffix (optional)", "generationMastrRoleSuffix", "text", "");
    mastrRoleSuffix.input.maxLength = 2;
    mastrRoleSuffix.input.pattern = "[A-Za-z]{2}";
    mastrRoleSuffix.input.autocapitalize = "characters";
    mastrPrefix.input.addEventListener("input", () => { mastrPrefix.input.value = mastrPrefix.input.value.toUpperCase(); });
    mastrRoleSuffix.input.addEventListener("input", () => { mastrRoleSuffix.input.value = mastrRoleSuffix.input.value.toUpperCase(); });
    mastrRoleSuffix.input.setAttribute("aria-describedby", "generationMastrRoleHelp");
    const roleHelp = element(
      "span",
      "help-text",
      "Rollen-Suffixe sind nur für passende Marktteilnehmer-Präfixe zulässig; die API prüft die Kombination.",
    );
    roleHelp.id = "generationMastrRoleHelp";
    mastrRoleSuffix.wrapper.append(roleHelp);
    fields.append(mastrPrefix.wrapper, mastrRoleSuffix.wrapper);
  }

  form.append(fields);
  const buttons = element("div", "button-row");
  const submit = element("button", "primary-button", "Werte erzeugen");
  submit.type = "submit";
  buttons.append(submit);
  form.append(buttons);
  const resultHost = element("div", "result-area");
  formCard.append(form, resultHost);

  const requestCard = requestPreviewCard();
  const requestPre = requestCard.querySelector("pre");
  const requestBody = () => compactObject({
    profile: profile.input.value || undefined,
    count: Number(count.input.value),
    fixture_seed: seed.input.value.trim() || undefined,
    format: format.input.value,
    sector: sector?.input.value || undefined,
    country: country?.input.value.trim().toUpperCase() || undefined,
    include_branch: includeBranch ? includeBranch.input.value === "true" : undefined,
    invoice_reference: invoiceReference?.input.value.trim() || undefined,
    prefix: mastrPrefix?.input.value.trim().toUpperCase() || undefined,
    role_suffix: mastrRoleSuffix?.input.value.trim().toUpperCase() || undefined,
  });
  const updatePreview = () => renderRequestPreview(requestPre, operation, requestBody());
  form.addEventListener("input", updatePreview);
  form.addEventListener("change", updatePreview);
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    await submitOperation({ operation, body: requestBody(), submit, resultHost, renderer: renderGenerationResult });
  });
  updatePreview();
  layout.append(formCard, requestCard);
  elements.detailPanel.append(layout);
}

function renderValidationPanel(identifier, action) {
  const operation = operationFor(identifier, action) ?? operationFor(identifier, "validate");
  const layout = element("div", "action-layout");
  const formCard = element("section", "panel-card");
  const form = element("form");
  form.append(element("h2", null, action === "parse" ? "Kennung zerlegen" : "Kennung validieren"));
  const id = inputField("Kennung", `${action}Value`, "text", "");
  id.wrapper.classList.add("is-wide");
  id.input.autocomplete = "off";
  id.input.spellcheck = false;
  id.input.required = true;
  form.append(id.wrapper);
  form.append(element("p", "help-text", action === "parse"
    ? "Die Zerlegung verwendet die vom Katalog angegebene Parse- oder Validierungsoperation und zeigt alle gelieferten Bestandteile."
    : "Format, Prüfziffer, Verzeichnis und Vergabestatus werden bewusst getrennt ausgewiesen."));
  const buttons = element("div", "button-row");
  const submit = element("button", "primary-button", action === "parse" ? "Zerlegen" : "Prüfen");
  submit.type = "submit";
  buttons.append(submit);
  form.append(buttons);
  const resultHost = element("div", "result-area");
  formCard.append(form, resultHost);

  const requestCard = requestPreviewCard();
  const requestPre = requestCard.querySelector("pre");
  const requestBody = () => ({ id: id.input.value.trim() });
  const updatePreview = () => renderRequestPreview(requestPre, operation, requestBody());
  id.input.addEventListener("input", updatePreview);
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    await submitOperation({ operation, body: requestBody(), submit, resultHost, renderer: (result, host) => renderValidationResult(result, host, action === "parse") });
  });
  updatePreview();
  layout.append(formCard, requestCard);
  elements.detailPanel.append(layout);
}

function renderNegativePanel(identifier) {
  const operation = negativeOperation(identifier);
  const layout = element("div", "action-layout");
  const formCard = element("section", "panel-card");
  const form = element("form");
  form.append(element("h2", null, "Gezielt ungültigen Testwert erzeugen"));
  const mutations = [
    { value: "length", label: "Falsche Länge" },
    { value: "character_set", label: "Ungültige Zeichenmenge" },
  ];
  if (identifier.checksum_scheme) mutations.unshift({ value: "checksum", label: "Falsche Prüfziffer" });
  const mutation = selectField("Fehlerart", "negativeMutation", mutations);
  const seed = inputField("Fixture-Seed", "negativeSeed", "text", "negative-test-1");
  const profile = selectField("Ausgangsprofil", "negativeProfile", (identifier.generation_profiles ?? []).map((value) => ({ value, label: labelProfile(value) })));
  profile.input.value = identifier.default_profile ?? identifier.generation_profiles?.[0] ?? "";
  form.append(mutation.wrapper, profile.wrapper, seed.wrapper);
  let country;
  if (identifier.slug === "iban") {
    country = inputField("IBAN-Land (ISO Alpha-2)", "negativeCountry", "text", "DE");
    country.input.maxLength = 2;
    country.input.pattern = "[A-Za-z]{2}";
    country.input.autocapitalize = "characters";
    country.input.addEventListener("input", () => {
      country.input.value = country.input.value.toUpperCase();
      syncInternationalIbanProfiles(country.input, profile.input);
    });
    profile.input.addEventListener("change", () => syncInternationalIbanProfiles(country.input, profile.input));
    syncInternationalIbanProfiles(country.input, profile.input);
    form.append(country.wrapper);
  }
  let sector;
  if (identifier.slug === "mp-id") {
    sector = selectField("Sparte", "negativeSector", [
      { value: "electricity", label: "Strom" },
      { value: "gas", label: "Gas" },
    ]);
    form.append(sector.wrapper);
  }
  const buttons = element("div", "button-row");
  const submit = element("button", "primary-button", "Negativfixture erzeugen");
  submit.type = "submit";
  buttons.append(submit);
  form.append(buttons);
  const resultHost = element("div", "result-area");
  formCard.append(form, resultHost);
  const requestCard = requestPreviewCard();
  const requestPre = requestCard.querySelector("pre");
  const body = () => ({
    mutation: mutation.input.value,
    fixture_seed: seed.input.value.trim() || undefined,
    profile: profile.input.value || undefined,
    country: country?.input.value.trim().toUpperCase() || undefined,
    sector: sector?.input.value || undefined,
  });
  const update = () => renderRequestPreview(requestPre, operation, compactObject(body()));
  form.addEventListener("input", update);
  form.addEventListener("change", update);
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    await submitOperation({ operation, body: compactObject(body()), submit, resultHost, renderer: renderNegativeResult });
  });
  update();
  layout.append(formCard, requestCard);
  elements.detailPanel.append(layout);
}

function renderLookupPanel(identifier) {
  const operation = operationFor(identifier, "lookup");
  const layout = element("div", "action-layout");
  const formCard = element("section", "panel-card");
  const form = element("form");
  form.append(element("h2", null, "Verzeichnis nachschlagen"));
  const id = inputField("Kennung", "lookupValue", "text", "");
  id.input.required = true;
  form.append(id.wrapper);
  const submit = element("button", "primary-button", "Nachschlagen");
  submit.type = "submit";
  form.append(element("div", "button-row", submit));
  const resultHost = element("div", "result-area");
  formCard.append(form, resultHost);
  const requestCard = requestPreviewCard();
  const requestPre = requestCard.querySelector("pre");
  const body = () => ({ id: id.input.value.trim() });
  id.input.addEventListener("input", () => renderRequestPreview(requestPre, operation, body()));
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    await submitOperation({ operation, body: body(), submit, resultHost, renderer: renderGenericResult });
  });
  renderRequestPreview(requestPre, operation, body());
  layout.append(formCard, requestCard);
  elements.detailPanel.append(layout);
}

async function submitOperation({ operation, body, submit, resultHost, renderer }) {
  if (!operation) {
    resultHost.replaceChildren(notice("Im Katalog ist keine passende Operation hinterlegt.", "error"));
    return;
  }
  submit.disabled = true;
  submit.setAttribute("aria-busy", "true");
  resultHost.replaceChildren(notice("Anfrage läuft …"));
  try {
    const response = await apiRequest(operation, body);
    const result = await parseResponse(response);
    if (!response.ok) throw new Error(result?.error ?? `HTTP ${response.status}`);
    state.result = result;
    resultHost.replaceChildren();
    renderer(result, resultHost);
  } catch (error) {
    resultHost.replaceChildren(notice(error instanceof Error ? error.message : String(error), "error"));
  } finally {
    submit.disabled = false;
    submit.removeAttribute("aria-busy");
  }
}

async function apiRequest(operation, body) {
  const method = String(operation.method ?? "post").toUpperCase();
  const options = { method, headers: { Accept: "application/json" } };
  if (method !== "GET" && method !== "HEAD") {
    options.headers["Content-Type"] = "application/json";
    options.body = JSON.stringify(body);
  }
  return fetch(operation.path, options);
}

async function parseResponse(response) {
  const contentType = response.headers.get("content-type") ?? "";
  if (contentType.includes("application/json")) return response.json();
  const text = await response.text();
  return text ? { error: text } : {};
}

function renderGenerationResult(result, host) {
  const items = Array.isArray(result?.items) ? result.items : [];
  const wrapper = element("div");
  wrapper.append(resultToolbar(`${items.length} Wert${items.length === 1 ? "" : "e"}`, result, items));
  if (!items.length) {
    wrapper.append(notice("Die API hat keine Werte zurückgegeben.", "error"));
  } else {
    const list = element("div", "result-list");
    list.append(...items.map(renderIdentifierResult));
    wrapper.append(list);
  }
  host.append(wrapper);
}

function renderIdentifierResult(item) {
  const card = element("article", "result-card");
  card.append(element("p", "result-value", item?.value ?? "–"));
  if (item?.formatted && item.formatted !== item.value) card.append(element("p", "help-text", `Formatiert: ${item.formatted}`));
  card.append(statusGrid(item));
  if (Array.isArray(item?.parts) && item.parts.length) card.append(partsTable(item.parts));
  appendMessages(card, item);
  return card;
}

function renderValidationResult(result, host, parseOnly) {
  const heading = result?.valid === true ? "Kennung ist formal gültig" : result?.valid === false ? "Kennung ist ungültig" : "Prüfergebnis";
  const wrapper = element("div");
  wrapper.append(resultToolbar(heading, result, result?.normalized ? [{ value: result.normalized }] : []));
  const card = element("article", "result-card");
  if (result?.normalized) card.append(element("p", "result-value", result.normalized));
  if (!parseOnly || result?.checks) card.append(statusGrid(result));
  if (Array.isArray(result?.parts) && result.parts.length) card.append(partsTable(result.parts));
  else if (parseOnly) card.append(notice("Die API hat keine zerlegten Bestandteile geliefert."));
  appendMessages(card, result);
  wrapper.append(card);
  host.append(wrapper);
}

function renderGenericResult(result, host) {
  const wrapper = element("div");
  wrapper.append(resultToolbar("Ergebnis", result, extractValues(result).map((value) => ({ value }))));
  const pre = element("pre", "request-preview json-result");
  pre.textContent = JSON.stringify(result, null, 2);
  wrapper.append(pre);
  host.append(wrapper);
}

function renderNegativeResult(result, host) {
  const original = result?.original;
  const mutatedValue = result?.mutated_value;
  const wrapper = element("div");
  wrapper.append(resultToolbar(
    "Negativfixture",
    result,
    mutatedValue ? [{ value: mutatedValue, kind: result?.kind }] : [],
  ));

  const card = element("article", "result-card");
  card.append(
    element("p", "eyebrow", `Mutation · ${humanize(result?.mutation)}`),
    element("p", "result-value", mutatedValue ?? "–"),
  );
  if (original?.value) card.append(element("p", "help-text", `Gültiges Ausgangsfixture: ${original.value}`));

  const verification = element("div", "status-grid");
  verification.append(
    compactStatus("Erwartet gültig", result?.expected_valid),
    compactStatus("Validator lehnt ab", result?.validator_rejected),
  );
  card.append(verification);
  if (original) {
    card.append(element("h4", null, "Status des Ausgangsfixtures"), statusGrid(original));
    appendMessages(card, original);
  }
  wrapper.append(card);
  host.append(wrapper);
}

function renderScenarioResult(result, host) {
  const scenarioItems = Array.isArray(result?.items) ? result.items : [];
  const identifiers = scenarioItems.map((item) => item?.identifier).filter(Boolean);
  const wrapper = element("div");
  wrapper.append(resultToolbar(
    `${scenarioItems.length} zusammengehörige Kennungen`,
    result,
    identifiers,
  ));

  const list = element("div", "result-list");
  for (const item of scenarioItems) {
    const card = renderIdentifierResult(item?.identifier);
    card.prepend(element("p", "eyebrow", identifierDisplayName(item?.key)));
    if (Array.isArray(item?.depends_on) && item.depends_on.length) {
      card.append(element("p", "help-text", `Abhängig von: ${item.depends_on.map(identifierDisplayName).join(", ")}`));
    }
    list.append(card);
  }
  if (list.childElementCount) wrapper.append(list);
  else wrapper.append(notice("Die API hat keine Szenariokennungen geliefert.", "error"));
  appendMessages(wrapper, result);
  host.append(wrapper);
}

function resultToolbar(title, result, items) {
  const toolbar = element("div", "result-toolbar");
  toolbar.append(element("h3", null, title));
  const copies = element("div", "copy-group");
  copies.append(
    copyButton("Text", () => valuesAsText(items)),
    copyButton("JSON", () => JSON.stringify(result, null, 2)),
    copyButton("CSV", () => valuesAsCsv(items)),
  );
  toolbar.append(copies);
  return toolbar;
}

function copyButton(label, valueFactory) {
  const button = element("button", "copy-button", `${label} kopieren`);
  button.type = "button";
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(valueFactory());
      showToast(`${label} wurde kopiert.`);
    } catch {
      showToast("Kopieren ist in diesem Browser nicht verfügbar.");
    }
  });
  return button;
}

function valuesAsText(items) {
  return (items ?? []).map((item) => typeof item === "string" ? item : item?.value ?? item?.normalized ?? "").filter(Boolean).join("\n");
}

function valuesAsCsv(items) {
  const normalized = (items ?? []).map((item) => typeof item === "string" ? { value: item } : item ?? {});
  const headers = [
    "value", "formatted", "kind", "profile", "synthetic", "production_usable",
    "account_existence", "collision_guarantee",
  ];
  const rows = normalized.map((item) => headers.map((header) => csvCell(item[header])).join(","));
  return [headers.join(","), ...rows].join("\n");
}

function csvCell(value) {
  if (value == null) return "";
  const text = String(value);
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

function statusGrid(item) {
  const grid = element("div", "status-grid");
  const statuses = [
    ["Format", item?.checks?.syntax],
    ["Prüfziffer", item?.checks?.checksum],
    ["Verzeichnis", item?.checks?.directory],
    ["Vergabe", item?.checks?.assignment ?? item?.allocation_status],
    ["Synthetisch", item?.synthetic],
    ["Produktiv nutzbar", item?.production_usable],
  ];
  if (item?.account_existence != null) statuses.push(["Kontoexistenz", item.account_existence]);
  if (item?.collision_guarantee != null) statuses.push(["Kollisionsgarantie", item.collision_guarantee]);
  for (const [label, value] of statuses) {
    const key = String(value ?? "unknown");
    const status = element("div", `status-item ${statusClass(key)}`);
    status.append(element("span", null, label), element("strong", null, checkLabels[key] ?? humanize(key)));
    grid.append(status);
  }
  return grid;
}

function compactStatus(label, value) {
  const key = String(value ?? "unknown");
  const status = element("div", `status-item ${statusClass(key)}`);
  status.append(element("span", null, label), element("strong", null, checkLabels[key] ?? humanize(key)));
  return status;
}

function statusClass(value) {
  if (["valid", "found", "true"].includes(value)) return "is-positive";
  if (["invalid", "false"].includes(value)) return "is-negative";
  return "is-neutral";
}

function partsTable(parts) {
  const table = element("table", "parts-table");
  const caption = element("caption", "sr-only", "Zerlegte Bestandteile");
  const head = element("thead");
  const headerRow = element("tr");
  headerRow.append(element("th", null, "Bestandteil"), element("th", null, "Wert"));
  head.append(headerRow);
  const body = element("tbody");
  for (const part of parts) {
    const row = element("tr");
    row.append(element("td", null, humanize(part?.name)), element("td", null, part?.value ?? "–"));
    body.append(row);
  }
  table.append(caption, head, body);
  return table;
}

function appendMessages(parent, data) {
  for (const [key, className] of [["warnings", "warning-list"], ["errors", "error-list"]]) {
    if (!Array.isArray(data?.[key]) || !data[key].length) continue;
    const list = element("ul", className);
    for (const message of data[key]) list.append(element("li", null, String(message)));
    parent.append(list);
  }
}

async function renderScenarios() {
  updateCurrentNavigation();
  if (!state.scenarios) await loadScenarios();
  if (!state.scenarios) return;
  renderScenarioContent();
}

async function loadScenarios() {
  elements.scenarioStatus.hidden = false;
  elements.scenarioContent.hidden = true;
  elements.scenarioStatus.className = "inline-notice";
  elements.scenarioStatus.textContent = "Szenariokatalog wird geladen …";
  try {
    const response = await fetch(SCENARIO_ENDPOINT, { headers: { Accept: "application/json" } });
    const data = await parseResponse(response);
    if (!response.ok) throw new Error(data?.error ?? `HTTP ${response.status}`);
    const scenarios = Array.isArray(data) ? data : data?.scenarios;
    if (!Array.isArray(scenarios)) throw new Error("Die API hat keine Szenarioliste geliefert.");
    state.scenarios = scenarios;
    state.selectedScenario = scenarios[0] ?? null;
    elements.scenarioStatus.hidden = true;
    elements.scenarioContent.hidden = false;
  } catch (error) {
    elements.scenarioStatus.classList.add("is-error");
    elements.scenarioStatus.textContent = `Szenarien sind noch nicht verfügbar: ${error instanceof Error ? error.message : String(error)}`;
  }
}

function renderScenarioContent() {
  const host = elements.scenarioContent;
  host.replaceChildren();
  if (!state.scenarios.length) {
    host.append(notice("Der Szenariokatalog ist leer."));
    return;
  }

  const layout = element("div", "action-layout");
  const formCard = element("section", "panel-card");
  const form = element("form");
  form.append(element("h2", null, "Szenario auswählen"));
  const scenarioList = element("div", "scenario-list");
  state.scenarios.forEach((scenario, index) => {
    const key = scenarioKey(scenario);
    const option = element("label", "scenario-option");
    const radio = document.createElement("input");
    radio.type = "radio";
    radio.name = "scenario";
    radio.value = key;
    radio.checked = key === scenarioKey(state.selectedScenario) || (!state.selectedScenario && index === 0);
    radio.addEventListener("change", () => {
      state.selectedScenario = scenario;
      syncScenarioSector();
      updateScenarioPreview();
    });
    option.append(radio, element("strong", null, scenario.label ?? scenario.name ?? humanize(key)));
    if (scenario.description) option.append(element("small", null, scenario.description));
    if (Array.isArray(scenario.identifiers) && scenario.identifiers.length) {
      const identifiers = chipList(scenario.identifiers.map((item) => identifierDisplayName(item?.kind ?? item)));
      identifiers.classList.add("scenario-identifiers");
      option.append(identifiers);
    }
    scenarioList.append(option);
  });
  form.append(scenarioList);

  const fields = element("div", "form-grid");
  const sector = selectField("Sparte", "scenarioSector", [
    { value: "electricity", label: "Strom" },
    { value: "gas", label: "Gas" },
  ]);
  const profiles = catalogProfiles();
  const profile = selectField("Profil", "scenarioProfile", profiles.map((value) => ({ value, label: labelProfile(value) })));
  if (profiles.includes("synthetic_non_routable")) profile.input.value = "synthetic_non_routable";
  const seed = inputField("Fixture-Seed", "scenarioSeed", "text", "nrg-demo-1");
  seed.wrapper.classList.add("is-wide");
  fields.append(sector.wrapper, profile.wrapper, seed.wrapper);
  form.append(fields);
  const submit = element("button", "primary-button", "Szenario erzeugen");
  submit.type = "submit";
  form.append(element("div", "button-row", submit));
  const resultHost = element("div", "result-area");
  formCard.append(form, resultHost);

  const requestCard = requestPreviewCard();
  const requestPre = requestCard.querySelector("pre");
  const operation = { path: `${SCENARIO_ENDPOINT}/generate`, method: "post" };
  const syncScenarioSector = () => {
    const allowedSectors = new Set(state.selectedScenario?.sectors ?? ["electricity", "gas"]);
    for (const option of sector.input.options) option.disabled = !allowedSectors.has(option.value);
    if (sector.input.selectedOptions[0]?.disabled) {
      sector.input.value = [...allowedSectors].find((value) => value === "electricity" || value === "gas") ?? "";
    }
  };
  const body = () => compactObject({
    scenario: scenarioKey(state.selectedScenario),
    sector: sector.input.value,
    profile: profile.input.value || undefined,
    fixture_seed: seed.input.value.trim() || undefined,
  });
  const updateScenarioPreview = () => renderRequestPreview(requestPre, operation, body());
  form.addEventListener("input", updateScenarioPreview);
  form.addEventListener("change", updateScenarioPreview);
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    await submitOperation({ operation, body: body(), submit, resultHost, renderer: renderScenarioResult });
  });
  syncScenarioSector();
  updateScenarioPreview();
  layout.append(formCard, requestCard);
  host.append(layout);
}

function scenarioKey(scenario) {
  return typeof scenario === "string" ? scenario : scenario?.scenario ?? scenario?.slug ?? scenario?.id ?? scenario?.name ?? "";
}

function catalogProfiles() {
  return [...new Set(state.identifiers.flatMap((identifier) => identifier.generation_profiles ?? []))];
}

function identifierDisplayName(kind) {
  const descriptor = state.identifiers.find((identifier) => identifier.kind === kind || identifier.slug === kind);
  return descriptor?.label ?? humanize(kind);
}

function extractValues(value) {
  const found = [];
  const visit = (node) => {
    if (!node || typeof node !== "object") return;
    if (typeof node.value === "string") found.push(node.value);
    if (Array.isArray(node)) node.forEach(visit);
    else Object.values(node).forEach(visit);
  };
  visit(value);
  return [...new Set(found)];
}

function operationFor(identifier, capability) {
  return identifier.operations.find((operation) => !operation.deprecated && operation.capability === capability)
    ?? identifier.operations.find((operation) => operation.capability === capability);
}

function negativeOperation(identifier) {
  return identifier.operations.find((operation) => {
    if (operation.deprecated) return false;
    const marker = `${operation.capability ?? ""} ${operation.operation_id ?? ""} ${operation.path ?? ""}`.toLowerCase();
    return marker.includes("negative");
  });
}

function primaryTag(identifier) {
  return identifier.operations.find((operation) => !operation.deprecated)?.primary_tag
    ?? identifier.operations[0]?.primary_tag
    ?? humanize(identifier.domain);
}

function useDescription(identifier) {
  if (typeof identifier.description === "string" && identifier.description.trim()) {
    return identifier.description;
  }
  const capabilities = (identifier.capabilities ?? []).map(labelCapability).join(", ").toLocaleLowerCase("de");
  return `${identifier.label} kann über die im Katalog veröffentlichten Operationen ${capabilities || "verarbeitet"} werden. Syntax- und Prüfzifferergebnisse treffen keine Aussage über eine reale Vergabe.`;
}

function requestPreviewCard() {
  const card = element("aside", "panel-card");
  card.append(element("h2", null, "API-Request"));
  const pre = element("pre", "request-preview");
  pre.setAttribute("aria-label", "Vollständiger API-Request");
  card.append(pre, element("p", "help-text", "Pfad und Methode stammen aus den Operationsmetadaten des Katalogs."));
  return card;
}

function renderRequestPreview(pre, operation, body) {
  if (!operation) {
    pre.textContent = "Keine öffentliche Operation im Katalog.";
    return;
  }
  const method = String(operation.method ?? "post").toUpperCase();
  const lines = [`${method} ${operation.path}`, "Accept: application/json"];
  if (method !== "GET" && method !== "HEAD") {
    lines.push("Content-Type: application/json", "", JSON.stringify(body, null, 2));
  }
  pre.textContent = lines.join("\n");
}

function inputField(label, id, type, value) {
  const wrapper = element("label", "field");
  wrapper.append(element("span", null, label));
  const input = document.createElement("input");
  input.id = id;
  input.type = type;
  input.value = value;
  wrapper.append(input);
  return { wrapper, input };
}

function selectField(label, id, options) {
  const wrapper = element("label", "field");
  wrapper.append(element("span", null, label));
  const input = document.createElement("select");
  input.id = id;
  for (const option of options) {
    const node = document.createElement("option");
    node.value = option.value;
    node.textContent = option.label;
    input.append(node);
  }
  wrapper.append(input);
  return { wrapper, input };
}

function metadataItem(term, description) {
  const wrapper = element("div", "metadata-item");
  wrapper.append(element("dt", null, term), element("dd", null, description || "–"));
  return wrapper;
}

function chipList(labels, accent = false) {
  const list = element("div", "chip-list");
  for (const label of labels.filter(Boolean)) list.append(element("span", `chip${accent ? " is-accent" : ""}`, label));
  return list;
}

function notice(message, kind = "") {
  return element("div", `inline-notice${kind ? ` is-${kind}` : ""}`, message);
}

function groupBy(items, keyFunction) {
  const groups = new Map();
  for (const item of items) {
    const key = keyFunction(item);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(item);
  }
  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b, "de"));
}

function sortByLabel(a, b) {
  return a.label.localeCompare(b.label, "de");
}

function labelCapability(value) { return capabilityLabels[value] ?? humanize(value); }
function labelProfile(value) { return profileLabels[value] ?? humanize(value); }

function syncInternationalIbanProfiles(countryInput, profileSelect) {
  const country = countryInput.value.trim().toUpperCase();
  const isInternational = country.length === 2 && country !== "DE";
  const internationalProfiles = new Set(["checksum_only", "official_example"]);
  for (const option of profileSelect.options) {
    option.disabled = isInternational && !internationalProfiles.has(option.value);
  }
  if (profileSelect.selectedOptions[0]?.disabled) {
    const fallback = [...profileSelect.options].find((option) => option.value === "checksum_only" && !option.disabled)
      ?? [...profileSelect.options].find((option) => !option.disabled);
    profileSelect.value = fallback?.value ?? "";
  }
}

function humanize(value) {
  if (value == null || value === "") return "–";
  return String(value).replaceAll("_", " ").replaceAll(".", " · ").replaceAll("-", " ").replace(/\b\p{L}/gu, (letter) => letter.toLocaleUpperCase("de"));
}

function compactObject(value) {
  return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined && item !== ""));
}

function element(tagName, className, ...children) {
  const node = document.createElement(tagName);
  if (className) node.className = className;
  for (const child of children) {
    if (child == null) continue;
    node.append(child instanceof Node ? child : textNode(String(child)));
  }
  return node;
}

function textNode(value) {
  return document.createTextNode(value);
}

function showToast(message) {
  clearTimeout(toastTimer);
  elements.toast.textContent = message;
  elements.toast.hidden = false;
  toastTimer = setTimeout(() => { elements.toast.hidden = true; }, 2400);
}
