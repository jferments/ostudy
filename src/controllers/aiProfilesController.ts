import { useCallback, useEffect, useState } from "react";

import type { ShowToast } from "../components/Toast";
import {
  clearAiProviderCredential,
  createAiProviderProfile,
  deleteAiProviderProfile,
  getAiProviderCatalog,
  getAiProviderProbeResult,
  getAiProviderProfile,
  getAiTaskDefaults,
  listAiProviderModels,
  listAiProviderProfiles,
  setAiProviderCredential,
  setFlashcardGenerationProfile,
  testAiProviderProfile,
  updateAiProviderProfile,
  type AiCredentialPersistence,
  type AiProfileDraft,
  type AiProviderCatalogEntry,
  type AiProviderKind,
  type AiProviderModel,
  type AiProviderProbeResult,
  type AiProviderProfileDetail,
  type AiProviderProfileSummary,
  type AiTaskDefaults,
} from "../ipc/aiProfiles";
import { getErrorMessage } from "../ipc/common";
import { useRequestLane } from "./requestLane";

const EMPTY_DEFAULTS: AiTaskDefaults = { flashcardGenerationProfileId: null };

export interface AiProfilesController {
  catalog: AiProviderCatalogEntry[];
  profiles: AiProviderProfileSummary[];
  defaults: AiTaskDefaults;
  selectedProfile: AiProviderProfileDetail | null;
  selectedProfileId: string | null;
  draft: AiProfileDraft | null;
  credentialDraft: string;
  credentialPersistence: AiCredentialPersistence;
  models: AiProviderModel[];
  modelsLoaded: boolean;
  probeResult: AiProviderProbeResult | null;
  dirty: boolean;
  loading: boolean;
  mutating: boolean;
  refreshingModels: boolean;
  testing: boolean;
  errorMessage: string | null;
  selectProfile: (profileId: string) => void;
  beginCreate: (providerKind: AiProviderKind) => void;
  updateDraft: (patch: Partial<AiProfileDraft>) => void;
  setCredentialDraft: (value: string) => void;
  setCredentialPersistence: (value: AiCredentialPersistence) => void;
  connectSetup: () => Promise<void>;
  saveProfile: () => Promise<void>;
  deleteProfile: () => Promise<void>;
  saveCredential: () => Promise<void>;
  clearCredential: () => Promise<void>;
  refreshModels: () => Promise<void>;
  testProfile: () => Promise<void>;
  setFlashcardDefault: (profileId: string | null) => Promise<void>;
  discardDraft: () => void;
  reload: () => Promise<void>;
}

export function useAiProfilesController(showToast: ShowToast): AiProfilesController {
  const [catalog, setCatalog] = useState<AiProviderCatalogEntry[]>([]);
  const [profiles, setProfiles] = useState<AiProviderProfileSummary[]>([]);
  const [defaults, setDefaults] = useState<AiTaskDefaults>(EMPTY_DEFAULTS);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [selectedProfile, setSelectedProfile] = useState<AiProviderProfileDetail | null>(null);
  const [draft, setDraft] = useState<AiProfileDraft | null>(null);
  const [credentialDraft, setCredentialDraft] = useState("");
  const [credentialPersistence, setCredentialPersistence] =
    useState<AiCredentialPersistence>("secure");
  const [models, setModels] = useState<AiProviderModel[]>([]);
  const [modelsLoaded, setModelsLoaded] = useState(false);
  const [probeResult, setProbeResult] = useState<AiProviderProbeResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [mutating, setMutating] = useState(false);
  const [refreshingModels, setRefreshingModels] = useState(false);
  const [testing, setTesting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const listLane = useRequestLane();
  const detailLane = useRequestLane();
  const modelLane = useRequestLane();

  const loadProfiles = useCallback(async (): Promise<AiProviderProfileSummary[]> => {
    const token = listLane.begin();
    const loaded = await listAiProviderProfiles();
    if (listLane.isCurrent(token)) setProfiles(loaded);
    return loaded;
  }, [listLane]);

  const discoverModelsForProfile = useCallback(
    async (profile: AiProviderProfileDetail, announceChanges: boolean): Promise<void> => {
      if (!canDiscoverModels(profile)) {
        setModels([]);
        setModelsLoaded(false);
        return;
      }
      const token = modelLane.begin();
      setRefreshingModels(true);
      setErrorMessage(null);
      try {
        const result = await listAiProviderModels(profile.id, profile.revision);
        if (!modelLane.isCurrent(token)) return;
        setModels(result.models);
        setModelsLoaded(true);

        const selectedStillAvailable = result.models.some((model) => model.id === profile.modelId);
        if (result.models.length === 1 && result.models[0]?.id !== profile.modelId) {
          const serverModelId = result.models[0]?.id;
          if (serverModelId) {
            setDraft((current) =>
              current
                ? {
                    ...current,
                    modelId: serverModelId,
                    reasoningEffort: compatibleReasoningEffort(
                      current.reasoningEffort,
                      result.models[0] ?? null,
                    ),
                  }
                : current,
            );
            if (announceChanges) {
              showToast("info", "The provider reports one available model. ostudy selected it.");
            }
          }
        } else if (profile.modelId && !selectedStillAvailable) {
          setDraft((current) => (current ? { ...current, modelId: "" } : current));
          if (announceChanges) {
            showToast(
              "info",
              "The saved model is not in the provider's current model list. Choose one of the reported models.",
            );
          }
        }
      } catch (error: unknown) {
        if (modelLane.isCurrent(token)) {
          setModels([]);
          setModelsLoaded(true);
          setErrorMessage(getErrorMessage(error));
        }
      } finally {
        if (modelLane.isCurrent(token)) setRefreshingModels(false);
      }
    },
    [modelLane, showToast],
  );

  const reload = useCallback(async (): Promise<void> => {
    const token = listLane.begin();
    setLoading(true);
    setErrorMessage(null);
    try {
      const [loadedCatalog, loadedProfiles, loadedDefaults] = await Promise.all([
        getAiProviderCatalog(),
        listAiProviderProfiles(),
        getAiTaskDefaults(),
      ]);
      if (!listLane.isCurrent(token)) return;
      setCatalog(loadedCatalog);
      setProfiles(loadedProfiles);
      setDefaults(loadedDefaults);
      setSelectedProfileId((current) => {
        if (current && loadedProfiles.some((profile) => profile.id === current)) return current;
        return loadedProfiles[0]?.id ?? null;
      });
    } catch (error: unknown) {
      if (listLane.isCurrent(token)) setErrorMessage(getErrorMessage(error));
    } finally {
      if (listLane.isCurrent(token)) setLoading(false);
    }
  }, [listLane]);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    const profileId = selectedProfileId;
    if (!profileId || profileId.startsWith("new:")) {
      setSelectedProfile(null);
      setModels([]);
      setModelsLoaded(false);
      setProbeResult(null);
      return;
    }
    const token = detailLane.begin();
    void Promise.all([getAiProviderProfile(profileId), getAiProviderProbeResult(profileId)])
      .then(([profile, probe]) => {
        if (!detailLane.isCurrent(token)) return;
        setSelectedProfile(profile);
        setDraft(draftFromProfile(profile));
        setProbeResult(probe);
        setCredentialDraft("");
        setCredentialPersistence(
          profile.credentialStatus.securePersistenceAvailable ? "secure" : "session",
        );
        setModels([]);
        setModelsLoaded(false);
        if (canDiscoverModels(profile)) {
          void discoverModelsForProfile(profile, false);
        }
      })
      .catch((error: unknown) => {
        if (detailLane.isCurrent(token)) setErrorMessage(getErrorMessage(error));
      });
  }, [detailLane, discoverModelsForProfile, selectedProfileId]);

  const profileDraftDirty = hasProfileDraftChanges(draft, selectedProfile, selectedProfileId);
  const dirty = credentialDraft !== "" || profileDraftDirty;

  function selectProfile(profileId: string): void {
    setSelectedProfileId(profileId);
    setCredentialDraft("");
    setModels([]);
    setModelsLoaded(false);
    setErrorMessage(null);
  }

  function beginCreate(providerKind: AiProviderKind): void {
    const provider = catalog.find((entry) => entry.providerKind === providerKind);
    if (!provider || provider.implementationStatus !== "available") return;
    setSelectedProfileId(`new:${providerKind}`);
    setSelectedProfile(null);
    setDraft({
      displayName: provider.displayName,
      providerKind,
      baseUrl: provider.defaultBaseUrl,
      modelId: "",
      reasoningEffort: "low",
      authMode: provider.authenticationRequired ? "bearer" : "none",
      enabled: true,
      allowInsecureHttp: false,
      requireZeroDataRetention: false,
      requestTimeoutMs: provider.defaultTimeoutMs,
    });
    setCredentialDraft("");
    setCredentialPersistence("secure");
    setModels([]);
    setModelsLoaded(false);
    setProbeResult(null);
    setErrorMessage(null);
  }

  function updateDraft(patch: Partial<AiProfileDraft>): void {
    setDraft((current) => (current ? { ...current, ...patch } : current));
  }

  async function connectSetup(): Promise<void> {
    if (!draft || mutating) return;
    if (
      draft.authMode !== "none" &&
      credentialDraft === "" &&
      !profileHasCredential(selectedProfile)
    ) {
      setErrorMessage("Enter the API key before connecting to this provider.");
      return;
    }

    setMutating(true);
    setErrorMessage(null);
    let committedProfile: AiProviderProfileDetail | null = selectedProfile;
    let credentialCommitted = false;
    try {
      const wasExisting = selectedProfile !== null;
      let saved: AiProviderProfileDetail;
      if (!selectedProfile) {
        saved = await createAiProviderProfile(draft);
      } else if (profileDraftDirty) {
        saved = await updateAiProviderProfile(selectedProfile.id, selectedProfile.revision, draft);
      } else {
        saved = selectedProfile;
      }
      committedProfile = saved;

      if (saved.authMode !== "none" && credentialDraft !== "") {
        saved = await setAiProviderCredential(
          saved.id,
          saved.revision,
          credentialDraft,
          credentialPersistence,
        );
        committedProfile = saved;
        credentialCommitted = true;
      }

      setSelectedProfile(saved);
      setSelectedProfileId(saved.id);
      setDraft(draftFromProfile(saved));
      setCredentialDraft("");
      setProbeResult(null);
      setModels([]);
      setModelsLoaded(false);
      await loadProfiles();
      showToast("success", wasExisting ? "AI connection updated." : "AI connection saved.");
      if (canDiscoverModels(saved)) await discoverModelsForProfile(saved, false);
    } catch (error: unknown) {
      if (committedProfile && committedProfile !== selectedProfile) {
        setSelectedProfile(committedProfile);
        setSelectedProfileId(committedProfile.id);
        setDraft(draftFromProfile(committedProfile));
        setProbeResult(null);
        setModels([]);
        setModelsLoaded(false);
      }
      if (credentialCommitted) setCredentialDraft("");
      setErrorMessage(getErrorMessage(error));
    } finally {
      setMutating(false);
    }
  }

  async function saveProfile(): Promise<void> {
    if (!draft || mutating) return;
    setMutating(true);
    setErrorMessage(null);
    try {
      const wasExisting = selectedProfile !== null;
      const saved = selectedProfile
        ? await updateAiProviderProfile(selectedProfile.id, selectedProfile.revision, draft)
        : await createAiProviderProfile(draft);
      setSelectedProfile(saved);
      setDraft(draftFromProfile(saved));
      setSelectedProfileId(saved.id);
      setModels([]);
      setModelsLoaded(false);
      await loadProfiles();
      showToast("success", wasExisting ? "AI provider profile saved." : "AI provider profile created.");
      if (canDiscoverModels(saved)) {
        await discoverModelsForProfile(saved, !wasExisting);
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setMutating(false);
    }
  }

  async function deleteProfile(): Promise<void> {
    if (!selectedProfile || mutating) return;
    setMutating(true);
    setErrorMessage(null);
    try {
      await deleteAiProviderProfile(selectedProfile.id, selectedProfile.revision);
      const remaining = await loadProfiles();
      setSelectedProfileId(remaining[0]?.id ?? null);
      setSelectedProfile(null);
      setDraft(null);
      setCredentialDraft("");
      setModels([]);
      setModelsLoaded(false);
      setProbeResult(null);
      if (defaults.flashcardGenerationProfileId === selectedProfile.id) {
        setDefaults(EMPTY_DEFAULTS);
      }
      showToast("success", "AI provider profile deleted.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setMutating(false);
    }
  }

  async function saveCredential(): Promise<void> {
    if (!selectedProfile || credentialDraft === "" || mutating) return;
    setMutating(true);
    setErrorMessage(null);
    try {
      const saved = await setAiProviderCredential(
        selectedProfile.id,
        selectedProfile.revision,
        credentialDraft,
        credentialPersistence,
      );
      setSelectedProfile(saved);
      setDraft(draftFromProfile(saved));
      setCredentialDraft("");
      setModels([]);
      setModelsLoaded(false);
      setProbeResult(null);
      await loadProfiles();
      showToast(
        "success",
        credentialPersistence === "secure"
          ? "AI credential saved securely."
          : "AI credential is available for this session.",
      );
      await discoverModelsForProfile(saved, true);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setMutating(false);
    }
  }

  async function clearCredential(): Promise<void> {
    if (!selectedProfile || mutating) return;
    setMutating(true);
    setErrorMessage(null);
    try {
      const saved = await clearAiProviderCredential(selectedProfile.id, selectedProfile.revision);
      setSelectedProfile(saved);
      setDraft(draftFromProfile(saved));
      setCredentialDraft("");
      setModels([]);
      setModelsLoaded(false);
      setProbeResult(null);
      await loadProfiles();
      showToast("success", "AI credential removed.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setMutating(false);
    }
  }

  async function refreshModels(): Promise<void> {
    if (
      !selectedProfile ||
      dirty ||
      refreshingModels ||
      !canDiscoverModels(selectedProfile)
    ) {
      return;
    }
    await discoverModelsForProfile(selectedProfile, true);
  }

  async function testProfile(): Promise<void> {
    if (!selectedProfile || dirty || testing) return;
    setTesting(true);
    setErrorMessage(null);
    try {
      const result = await testAiProviderProfile(selectedProfile.id, selectedProfile.revision);
      setProbeResult(result);
      showToast(
        result.overallStatus === "failed" ? "error" : "success",
        result.overallStatus === "verified"
          ? "AI provider profile verified."
          : result.overallStatus === "partial"
            ? "AI provider responded with partial capability support."
            : "AI provider verification failed.",
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setTesting(false);
    }
  }

  async function setFlashcardDefault(profileId: string | null): Promise<void> {
    if (mutating) return;
    setMutating(true);
    setErrorMessage(null);
    try {
      const saved = await setFlashcardGenerationProfile(profileId);
      setDefaults(saved);
      showToast("success", "Flashcard-generation profile updated.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setMutating(false);
    }
  }

  function discardDraft(): void {
    setCredentialDraft("");
    if (selectedProfile) {
      setDraft(draftFromProfile(selectedProfile));
    } else {
      setDraft(null);
      setSelectedProfileId(profiles[0]?.id ?? null);
    }
  }

  return {
    catalog,
    profiles,
    defaults,
    selectedProfile,
    selectedProfileId,
    draft,
    credentialDraft,
    credentialPersistence,
    models,
    modelsLoaded,
    probeResult,
    dirty,
    loading,
    mutating,
    refreshingModels,
    testing,
    errorMessage,
    selectProfile,
    beginCreate,
    updateDraft,
    setCredentialDraft,
    setCredentialPersistence,
    connectSetup,
    saveProfile,
    deleteProfile,
    saveCredential,
    clearCredential,
    refreshModels,
    testProfile,
    setFlashcardDefault,
    discardDraft,
    reload,
  };
}

function canDiscoverModels(profile: AiProviderProfileDetail): boolean {
  if (profile.authMode === "none") return true;
  return profileHasCredential(profile);
}

function profileHasCredential(profile: AiProviderProfileDetail | null): boolean {
  if (!profile) return false;
  return (
    profile.credentialStatus.status === "saved_securely" ||
    profile.credentialStatus.status === "session_only"
  );
}

function hasProfileDraftChanges(
  draft: AiProfileDraft | null,
  selectedProfile: AiProviderProfileDetail | null,
  selectedProfileId: string | null,
): boolean {
  if (!draft) return false;
  if (!selectedProfile) return selectedProfileId?.startsWith("new:") === true;
  return JSON.stringify(draft) !== JSON.stringify(draftFromProfile(selectedProfile));
}

function draftFromProfile(profile: AiProviderProfileDetail): AiProfileDraft {
  return {
    displayName: profile.displayName,
    providerKind: profile.providerKind,
    baseUrl: profile.baseUrl,
    modelId: profile.modelId,
    reasoningEffort: profile.reasoningEffort,
    authMode: profile.authMode,
    enabled: profile.enabled,
    allowInsecureHttp: profile.allowInsecureHttp,
    requireZeroDataRetention: profile.requireZeroDataRetention ?? false,
    requestTimeoutMs: profile.requestTimeoutMs,
  };
}

function compatibleReasoningEffort(
  current: AiProfileDraft["reasoningEffort"],
  model: AiProviderModel | null,
): AiProfileDraft["reasoningEffort"] {
  if (current === "automatic") return current;
  const reasoning = model?.reasoning;
  if (!reasoning) return current;
  if (reasoning.mandatory && current === "none") return "automatic";
  const supported = reasoning.supportedEfforts;
  if (supported && !supported.includes(current)) return "automatic";
  return current;
}
