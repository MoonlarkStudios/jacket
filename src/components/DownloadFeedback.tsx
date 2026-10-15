import { Alert, Button, Group, Progress, Text } from '@mantine/core';
import { IconLoader2 } from '@tabler/icons-react';
import type { JobEvent } from '../domain';
import {
  getDownloadPresentation,
  presentOperationError,
} from '../operationPresentation';

interface DownloadFeedbackProps {
  progress: JobEvent | null;
  exporting: boolean;
  cancelling: boolean;
  destinationPath?: string | null;
  onOpenSettings: (trigger: HTMLButtonElement) => void;
  onChooseDestination?: () => void;
}

function ErrorDetails({ error }: { error: string | null }) {
  if (!error) return null;
  return (
    <details className="source-notes">
      <summary>View error details</summary>
      <Text
        size="xs"
        mt={8}
        style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}
      >
        {error}
      </Text>
    </details>
  );
}

export function DownloadFeedback({
  progress,
  exporting,
  cancelling,
  destinationPath,
  onOpenSettings,
  onChooseDestination,
}: DownloadFeedbackProps) {
  if (!progress) return null;
  const presentation = getDownloadPresentation(progress);
  if (exporting) {
    const title = cancelling ? 'Stopping download…' : presentation.title;
    return (
      <div className="export-progress">
        <Group justify="space-between" gap={8} mb={8}>
          <Group gap={8} wrap="nowrap" role="status">
            {!cancelling && (
              <IconLoader2 size={16} className="spinner" aria-hidden="true" />
            )}
            <Text size="sm" fw={600}>
              {title}
            </Text>
          </Group>
          <Text size="xs" c="dimmed">
            {presentation.countLabel}
          </Text>
        </Group>
        {presentation.indeterminate ? (
          <div
            className="operation-indeterminate"
            role="progressbar"
            aria-label={title}
            aria-valuetext={presentation.countLabel}
          >
            <span />
          </div>
        ) : (
          <Progress
            value={presentation.percentage}
            aria-label="Tracks prepared"
            aria-valuetext={presentation.countLabel}
            size={3}
          />
        )}
        <Text
          size="sm"
          c="dimmed"
          mt={8}
          className="operation-track-name"
          style={{ overflowWrap: 'anywhere' }}
        >
          {cancelling
            ? 'Downloaded audio is kept so you can try again.'
            : progress.stage === 'packaging'
              ? 'Audio and artwork are ready. Saving your ZIP…'
              : presentation.currentTrack || 'Preparing your selected tracks…'}
        </Text>
        {!cancelling && progress.message.startsWith('Retrying ') && (
          <Text size="xs" c="dimmed" mt={4}>
            Reconnecting after a connection problem…
          </Text>
        )}
      </div>
    );
  }
  if (!['failed', 'interrupted', 'cancelled'].includes(progress.stage))
    return null;
  const failure = presentOperationError(progress.error);
  const failed = progress.stage === 'failed';
  return (
    <div className="export-feedback">
      <Text size="sm" fw={600} role="status">
        {failed
          ? failure.title
          : progress.stage === 'interrupted'
            ? 'Your download is ready to resume'
            : 'Download stopped'}
      </Text>
      <Text size="sm" c="dimmed" mt={4}>
        {failed
          ? failure.guidance
          : 'Your selection is saved. Downloaded audio will be reused when you try again.'}
      </Text>
      <Text size="xs" c="dimmed" mt={6}>
        {presentation.countLabel}
        {failed && ' · Downloaded audio is kept for retry.'}
      </Text>
      {destinationPath && (
        <Text size="xs" c="dimmed" mt={6} style={{ overflowWrap: 'anywhere' }}>
          Retry saves to {destinationPath}
        </Text>
      )}
      {failed && (
        <>
          <Group gap="xs" mt={6}>
            {(failure.kind === 'session' || failure.kind === 'tools') && (
              <Button
                size="xs"
                variant="subtle"
                onClick={(event) => onOpenSettings(event.currentTarget)}
              >
                {failure.kind === 'session'
                  ? 'Check browser session'
                  : 'Check audio downloader'}
              </Button>
            )}
            {(failure.kind === 'destination' || failure.kind === 'storage') &&
              onChooseDestination && (
                <Button
                  size="xs"
                  variant="subtle"
                  onClick={onChooseDestination}
                >
                  Choose another folder
                </Button>
              )}
          </Group>
          <ErrorDetails error={progress.error} />
        </>
      )}
    </div>
  );
}

export function SourceErrorMessage({
  error,
  dismiss,
  session,
  onRetry,
}: {
  error: string;
  dismiss: () => void;
  session?: (trigger: HTMLButtonElement) => void;
  onRetry?: () => void;
}) {
  const presentation = presentOperationError(error);
  return (
    <Alert
      color="red"
      title={presentation.title}
      withCloseButton
      onClose={dismiss}
      mt="lg"
    >
      <Text size="sm">{presentation.guidance}</Text>
      <Group gap="xs" mt="xs">
        {onRetry && (
          <Button variant="subtle" size="xs" onClick={onRetry}>
            Try again
          </Button>
        )}
        {session &&
          (presentation.kind === 'session' ||
            presentation.kind === 'tools') && (
            <Button
              variant="subtle"
              size="xs"
              onClick={(event) => session(event.currentTarget)}
            >
              {presentation.kind === 'session'
                ? 'Check browser session'
                : 'Check audio downloader'}
            </Button>
          )}
      </Group>
      <ErrorDetails error={error} />
    </Alert>
  );
}
