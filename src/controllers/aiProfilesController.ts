import { useCallback, useEffect, useMemo, useState } from "react";

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
      setProbeResult(null);
      return;
    }
    const token = detailLane.begin();
    setErrorMessage(null);
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
      })
      .catch((error: unknown) => {
        if (detailLane.isCurrent(token)) setErrorMessage(getErrorMessage(error));
      });
  }, [detailLane, selectedProfileId]);

  const dirty = useMemo(() => {
    if (credentialDraft !== "") return true;
    if (!draft) return false;
    if (!selectedProfile) return selectedProfileId?.startsWith("new:") === true;
    return JSON.stringify(draft) !== JSON.stringify(draftFromProfile(selectedProfile));
  }, [credentialDraft, draft, selectedProfile, selectedProfileId]);

  function selectProfile(profileId: string): void {
    setSelectedProfileId(profileId);
    setCredentialDraft("");
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
      authMode: provider.authenticationRequired ? "bearer" : "none",
      enabled: true,
      allowInsecureHttp: false,
      requestTimeoutMs: provider.defaultTimeoutMs,
    });
    setCredentialDraft("");
    setCredentialPersistence("secure");
    setModels([]);
    setProbeResult(null);
    setErrorMessage(null);
  }

  function updateDraft(patch: Partial<AiProfileDraft>): void {
    setDraft((current) => (current ? { ...current, ...patch } : current));
  }

  async function saveProfile(): Promise<void> {
    if (!draft || mutating) return;
    setMutating(true);
    setErrorMessage(null);
    try {
      const saved = selectedProfile
        ? await updateAiProviderProfile(selectedProfile.id, selectedProfile.revision, draft)
        : await createAiProviderProfile(draft);
      setSelectedProfile(saved);
      setDraft(draftFromProfile(saved));
      setSelectedProfileId(saved.id);
      await loadProfiles();
      showToast("success", selectedProfile ? "AI provider profile saved." : "AI provider profile created.");
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
      setProbeResult(null);
      await loadProfiles();
      showToast(
        "success",
        credentialPersistence === "secure"
          ? "AI credential saved securely."
          : "AI credential is available for this session.",
      );
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
    if (!selectedProfile || dirty || refreshingModels) return;
    const token = modelLane.begin();
    setRefreshingModels(true);
    setErrorMessage(null);
    try {
      const result = await listAiProviderModels(selectedProfile.id, selectedProfile.revision);
      if (modelLane.isCurrent(token)) setModels(result.models);
    } catch (error: unknown) {
      if (modelLane.isCurrent(token)) setErrorMessage(getErrorMessage(error));
    } finally {
      if (modelLane.isCurrent(token)) setRefreshingModels(false);
    }
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

function draftFromProfile(profile: AiProviderProfileDetail): AiProfileDraft {
  return {
    displayName: profile.displayName,
    providerKind: profile.providerKind,
    baseUrl: profile.baseUrl,
    modelId: profile.modelId,
    authMode: profile.authMode,
    enabled: profile.enabled,
    allowInsecureHttp: profile.allowInsecureHttp,
    requestTimeoutMs: profile.requestTimeoutMs,
  };
}
