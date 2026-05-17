import { useCallback, useEffect, useReducer, useState } from 'react';
import { commands, events, type Model } from '../../shared/ipc/bindings';
import { initialState, type ManagerState, reduce } from './downloadManager';

export interface UseInstallingModelsResult {
  models: Model[];
  state: ManagerState;
  downloadOne: (modelId: string) => Promise<void>;
}

export function useInstallingModels(): UseInstallingModelsResult {
  const [models, setModels] = useState<Model[]>([]);
  const [state, dispatch] = useReducer(reduce, initialState);

  useEffect(() => {
    void (async () => {
      const list = await commands.listModels();
      if (list.status === 'error') {
        console.error('[models] listModels failed', list.error);
        return;
      }
      setModels(list.data);

      const status = await commands.installationStatus();
      if (status.status === 'error') {
        console.error('[models] installationStatus failed', status.error);
        return;
      }
      for (const [id, st] of status.data.items) {
        dispatch({ type: 'installation-checked', modelId: id, status: st });
      }
    })();
  }, []);

  useEffect(() => {
    let cancelled = false;
    const unlistens: Array<() => void> = [];

    void (async () => {
      const onProgress = await events.modelDownloadProgress.listen((ev) => {
        dispatch({
          type: 'progress',
          modelId: ev.payload.model_id,
          bytesDownloaded: ev.payload.downloaded,
          bytesTotal: ev.payload.total,
        });
      });
      const onCompleted = await events.modelDownloadCompleted.listen((ev) => {
        dispatch({ type: 'completed', modelId: ev.payload.model_id });
      });
      const onFailed = await events.modelDownloadFailed.listen((ev) => {
        dispatch({
          type: 'failed',
          modelId: ev.payload.model_id,
          message: ev.payload.message,
        });
      });

      if (cancelled) {
        onProgress();
        onCompleted();
        onFailed();
        return;
      }
      unlistens.push(onProgress, onCompleted, onFailed);
    })();

    return () => {
      cancelled = true;
      for (const u of unlistens) u();
    };
  }, []);

  const downloadOne = useCallback(async (modelId: string) => {
    dispatch({ type: 'download-clicked', modelId });
    const result = await commands.downloadModel(modelId);
    if (result.status === 'error') {
      dispatch({ type: 'failed', modelId, message: result.error });
    }
  }, []);

  return { models, state, downloadOne };
}
