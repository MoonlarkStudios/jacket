import { Alert, Button, Group, Stack, Text, TextInput } from '@mantine/core';
import { useState } from 'react';
import { useOwnedSession } from '../useOwnedSession';
import type { Workspace } from '../useWorkspace';

export function OwnedSessionSettings({
  workspace: w,
}: {
  workspace: Workspace;
}) {
  const session = useOwnedSession();
  const [url, setUrl] = useState('');
  if (!session.status?.supported) return null;
  const status = session.status;
  return (
    <Stack gap="sm">
      <Text size="sm" fw={600}>
        Jacket YouTube session · prototype
      </Text>
      <Text size="xs" c="dimmed">
        Sign in using a separate Chrome profile created for Jacket. Account use
        with the downloader can lead to account restrictions.
      </Text>
      <Text size="sm">{status.message}</Text>
      {session.error && <Alert color="red">{session.error}</Alert>}
      <Group>
        <Button
          variant="default"
          disabled={w.busy}
          onClick={() => {
            void session.act('connect');
          }}
        >
          {status.sessionId || status.signingIn
            ? 'Reconnect YouTube'
            : 'Connect YouTube'}
        </Button>
        {status.signingIn && (
          <Button
            loading={session.busy}
            disabled={w.busy}
            onClick={() => void session.act('save')}
          >
            I’ve signed in · save session
          </Button>
        )}
        <Button
          variant="subtle"
          color="red"
          disabled={session.busy}
          onClick={() => {
            void session.act('disconnect');
          }}
        >
          Disconnect
        </Button>
      </Group>
      {status.sessionId && (
        <>
          <TextInput
            label="Account-only test link"
            description="Use a private playlist or another item this account can access. Verification also checks that public access fails."
            value={url}
            onChange={(event) => setUrl(event.currentTarget.value)}
            disabled={w.busy}
          />
          <Button
            variant="default"
            disabled={w.busy || !url.trim()}
            loading={session.busy}
            onClick={() => void session.act('verify', url)}
          >
            Verify account access
          </Button>
        </>
      )}
      <Text size="xs" c="dimmed">
        Disconnect removes Jacket’s saved credentials, sign-in profile, and
        unfinished account-backed downloads. Exported archives remain. Google’s
        remote session must be signed out separately.
      </Text>
    </Stack>
  );
}
