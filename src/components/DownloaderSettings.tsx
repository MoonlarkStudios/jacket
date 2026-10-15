import {
  Accordion,
  Button,
  Group,
  Modal,
  SegmentedControl,
  Select,
  Stack,
  Switch,
  Text,
  TextInput,
} from '@mantine/core';
import {
  IconAlertCircle,
  IconArrowLeft,
  IconCheck,
  IconChevronRight,
  IconLoader2,
} from '@tabler/icons-react';
import { useEffect, useState } from 'react';
import { chooseDownloader, desktopAvailable } from '../desktop';
import type { DownloaderMode } from '../domain';
import { downloaderPresentation } from '../downloader';
import type { Workspace } from '../useWorkspace';

export function DownloaderStatusButton({
  workspace: w,
  open,
  autoFocus = false,
}: {
  workspace: Workspace;
  open: () => void;
  autoFocus?: boolean;
}) {
  const state = downloaderPresentation(
    w.downloader,
    w.downloaderPhase,
    w.downloaderError,
    w.cancellingDownloader,
  );
  return (
    <button
      type="button"
      className={`downloader-status is-${state.tone}`}
      aria-label={`Audio downloader settings: ${state.label}`}
      onClick={open}
      data-autofocus={autoFocus || undefined}
      disabled={w.browserSession.testing}
    >
      <span className="downloader-status-dot" aria-hidden="true" />
      <span>Audio downloader · {state.label}</span>
      <IconChevronRight size={14} aria-hidden="true" />
    </button>
  );
}

export function DownloaderSettings({
  workspace: w,
  opened,
  close,
  back,
}: {
  workspace: Workspace;
  opened: boolean;
  close: () => void;
  back: () => void;
}) {
  const d = w.downloader;
  const [mode, setMode] = useState<DownloaderMode>('managed');
  const [path, setPath] = useState('');
  const [browseError, setBrowseError] = useState<string | null>(null);
  const [browsing, setBrowsing] = useState(false);
  const [advanced, setAdvanced] = useState<string | null>(null);
  useEffect(() => {
    if (opened) {
      setMode(d?.mode ?? 'managed');
      setPath(d?.localPath || d?.detectedLocalPath || '');
      setBrowseError(null);
      setAdvanced(null);
    }
  }, [opened, d?.mode, d?.localPath, d?.detectedLocalPath]);
  const locked =
    w.busy ||
    w.checkingDownloader ||
    w.updatingDownloader ||
    w.configuringDownloader ||
    browsing ||
    !desktopAvailable;
  const state = downloaderPresentation(
    d,
    w.downloaderPhase,
    w.downloaderError,
    w.cancellingDownloader,
  );
  const message = browseError || w.downloaderError || d?.error;
  async function browse() {
    setBrowsing(true);
    setBrowseError(null);
    try {
      const value = await chooseDownloader(
        path || d?.detectedLocalPath || undefined,
      );
      if (value) setPath(value);
    } catch {
      setBrowseError(
        'Couldn’t open the file picker. You can paste the executable path instead.',
      );
    } finally {
      setBrowsing(false);
    }
  }
  return (
    <Modal
      returnFocus={false}
      opened={opened}
      onClose={close}
      title="Audio downloader"
      centered
      size="md"
    >
      <Stack gap="lg">
        <Text size="sm" c="dimmed">
          Jacket uses yt-dlp to keep downloads working as YouTube changes.
        </Text>
        <div
          className={`downloader-summary is-${state.tone}`}
          role="status"
          aria-live="polite"
        >
          <span className="downloader-summary-icon" aria-hidden="true">
            {state.tone === 'working' ? (
              <IconLoader2
                size={20}
                className={w.downloaderPhase ? 'spinner' : undefined}
              />
            ) : state.tone === 'attention' ? (
              <IconAlertCircle size={20} />
            ) : (
              <IconCheck size={20} />
            )}
          </span>
          <div>
            <Text size="sm" fw={550}>
              {state.label}
            </Text>
            <Text size="xs" c="dimmed" mt={3}>
              {d
                ? `Version ${d.installedVersion}${d.mode === 'local' ? ' · local install' : ''}`
                : desktopAvailable
                  ? 'Reading the installed version…'
                  : 'Available in the desktop app'}
            </Text>
          </div>
        </div>
        <SegmentedControl
          fullWidth
          aria-label="Audio downloader source"
          value={mode}
          disabled={locked}
          onChange={(value) => {
            setMode(value as DownloaderMode);
            setBrowseError(null);
            if (value === 'managed' && d?.mode === 'local')
              void w.setDownloaderPreferences({
                mode: 'managed',
                autoUpdate: d.autoUpdate,
                localPath: d.localPath,
              });
          }}
          data={[
            { value: 'managed', label: 'Automatic' },
            { value: 'local', label: 'Local install' },
          ]}
        />
        {mode === 'managed' ? (
          <Stack gap="md">
            <Switch
              label="Keep the downloader up to date"
              description="Checks daily when you open the app. Installs updates when downloads are idle."
              checked={d?.autoUpdate ?? true}
              disabled={locked || !d}
              onChange={(event) => {
                if (d)
                  void w.setDownloaderPreferences({
                    mode: 'managed',
                    autoUpdate: event.currentTarget.checked,
                    localPath: d.localPath,
                  });
              }}
            />
            {d?.latestVersion && d.updateAvailable && (
              <Text size="xs" c="dimmed">
                {d.pendingActivation ? 'Prepared update' : 'Latest release'}:{' '}
                {d.latestVersion}
              </Text>
            )}
            <Group justify="space-between" gap="sm">
              <Text size="xs" c="dimmed">
                {d?.checkedAt
                  ? `Last checked ${new Date(d.checkedAt * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}`
                  : 'No update check yet'}
              </Text>
              {w.updatingDownloader ? (
                <Button
                  variant="subtle"
                  size="xs"
                  loading={w.cancellingDownloader}
                  onClick={() => void w.cancelDownloaderUpdate()}
                >
                  Stop update
                </Button>
              ) : (
                <Button
                  variant="default"
                  size="xs"
                  loading={w.checkingDownloader}
                  disabled={locked}
                  onClick={() => void w.checkDownloader()}
                >
                  Check now
                </Button>
              )}
            </Group>
            {d?.updateAvailable &&
              (!d.autoUpdate || d.downgrade || !!message) &&
              !w.updatingDownloader && (
                <Button
                  variant="light"
                  size="sm"
                  disabled={locked}
                  onClick={() => void w.installDownloader()}
                >
                  {d.downgrade ? `Use latest ${d.channel}` : 'Update now'}
                </Button>
              )}
            <Accordion
              variant="default"
              value={advanced}
              onChange={setAdvanced}
            >
              <Accordion.Item value="advanced">
                <Accordion.Control>Advanced</Accordion.Control>
                <Accordion.Panel>
                  <Stack gap="md">
                    <Select
                      label="Release channel"
                      allowDeselect={false}
                      value={d?.channel ?? 'nightly'}
                      disabled={locked}
                      onChange={(value) => {
                        if (value)
                          void w.checkDownloader(value as 'stable' | 'nightly');
                      }}
                      data={[
                        {
                          value: 'nightly',
                          label: 'Nightly · newest YouTube fixes',
                        },
                        { value: 'stable', label: 'Stable · regular releases' },
                      ]}
                    />
                    <Text size="xs" c="dimmed">
                      Updates come from the official yt-dlp project and are
                      verified before use. Your current download always keeps
                      its version.
                    </Text>
                    {d?.usingUpdatedCopy && (
                      <button
                        type="button"
                        className="text-action"
                        disabled={locked}
                        onClick={() => void w.restoreBundledDownloader()}
                      >
                        Use the included version and pause updates
                      </button>
                    )}
                  </Stack>
                </Accordion.Panel>
              </Accordion.Item>
            </Accordion>
          </Stack>
        ) : (
          <form
            onSubmit={(event) => {
              event.preventDefault();
              if (!locked && path.trim())
                void w.setDownloaderPreferences({
                  mode: 'local',
                  autoUpdate: d?.autoUpdate ?? true,
                  localPath: path.trim(),
                });
            }}
          >
            <Stack gap="md">
              <Text size="sm" c="dimmed">
                Use a copy installed with Homebrew or another tool. Jacket will
                check that it works; updates stay with your package manager.
              </Text>
              <Group gap="sm" align="end" wrap="nowrap">
                <TextInput
                  label="Executable path"
                  placeholder="/opt/homebrew/bin/yt-dlp"
                  value={path}
                  onChange={(event) => setPath(event.currentTarget.value)}
                  disabled={locked}
                  style={{ flex: 1 }}
                  autoComplete="off"
                  spellCheck={false}
                />
                <Button
                  variant="default"
                  size="sm"
                  disabled={locked}
                  loading={browsing}
                  onClick={() => void browse()}
                >
                  Browse…
                </Button>
              </Group>
              <Button
                type="submit"
                variant="light"
                loading={w.configuringDownloader}
                disabled={locked || !path.trim()}
              >
                {w.configuringDownloader
                  ? 'Checking executable…'
                  : d?.mode === 'local' && path.trim() === d.localPath
                    ? 'Check executable'
                    : 'Use this executable'}
              </Button>
              {d?.mode === 'local' && (
                <Text size="xs" c="dimmed">
                  Jacket leaves this executable and its updates to you.
                </Text>
              )}
            </Stack>
          </form>
        )}
        {message && (
          <Text
            size="xs"
            c="orange.9"
            role="alert"
            style={{ overflowWrap: 'anywhere' }}
          >
            {message}
          </Text>
        )}
        {w.busy && (
          <Text size="xs" c="dimmed">
            You can change the downloader after this album finishes.
          </Text>
        )}
        <Group justify="space-between">
          <Button
            variant="subtle"
            leftSection={<IconArrowLeft size={16} />}
            onClick={back}
          >
            Back to Settings
          </Button>
          <Button variant="default" onClick={close}>
            Done
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
