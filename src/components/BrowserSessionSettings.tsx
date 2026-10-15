import {
  Accordion,
  Alert,
  Button,
  Group,
  Select,
  Stack,
  Text,
  TextInput,
} from '@mantine/core';
import { useEffect, useState } from 'react';
import { desktopAvailable } from '../desktop';
import type { Browser } from '../domain';
import type { Workspace } from '../useWorkspace';

const names: Record<Browser, string> = {
  chrome: 'Chrome',
  firefox: 'Firefox',
  safari: 'Safari',
  edge: 'Edge',
};

export function BrowserSessionSettings({
  workspace: w,
}: {
  workspace: Workspace;
}) {
  const session = w.browserSession;
  const [showSetup, setShowSetup] = useState(!!w.browser);
  const [browser, setBrowser] = useState<Browser | null>(w.browser);
  const [profile, setProfile] = useState<string | null>(w.browserProfile);
  const [url, setUrl] = useState(w.inspection?.sourceUrl || w.url);
  useEffect(() => {
    if (desktopAvailable && browser) void session.refresh(browser);
  }, [browser, session.refresh]);
  const catalog =
    session.catalog?.browser === browser ? session.catalog.value : null;
  const available =
    !!profile &&
    !!catalog?.profiles.some((candidate) => candidate.id === profile);
  const result =
    session.result?.browser === browser &&
    session.result.profileId === profile &&
    session.result.url === url.trim()
      ? session.result.value
      : null;
  const selected =
    w.browser === browser && w.browserProfile === profile && available;
  const activeProfile =
    session.catalog?.browser === w.browser
      ? session.catalog.value.profiles.find(
          (candidate) => candidate.id === w.browserProfile,
        )
      : null;
  return (
    <Stack gap="sm">
      <Text size="sm" fw={600}>
        Browser session
      </Text>
      <Text size="sm" c="dimmed">
        Public albums and playlists work without signing in.
      </Text>
      {!showSetup && (
        <Button
          variant="default"
          disabled={!desktopAvailable || w.busy}
          onClick={() => setShowSetup(true)}
          style={{ alignSelf: 'flex-start' }}
        >
          Use a browser session…
        </Button>
      )}
      {!desktopAvailable && (
        <Text size="sm" c="dimmed">
          Browser sessions are available in the desktop app.
        </Text>
      )}
      {showSetup && (
        <>
          <Text size="sm" c="dimmed">
            For private or restricted audio, choose the profile where you use
            YouTube.
          </Text>
          {w.browser && (
            <Alert
              color="blue"
              title={`Using ${names[w.browser]}${activeProfile ? ` · ${activeProfile.label}` : ''}`}
            >
              {w.browserProfile
                ? 'Jacket will read this profile when finding tracks or downloading.'
                : 'Choose the profile for this unfinished download before continuing.'}
              <Button
                mt="xs"
                size="xs"
                variant="subtle"
                disabled={w.busy}
                onClick={() => {
                  session.stop();
                  setBrowser(null);
                  setProfile(null);
                  setShowSetup(false);
                  session.clearResult();
                }}
              >
                Stop using this session
              </Button>
            </Alert>
          )}
          <Select
            label="Browser"
            placeholder="Choose a browser"
            value={browser}
            allowDeselect={false}
            data={Object.entries(names).map(([value, label]) => ({
              value,
              label,
            }))}
            disabled={!desktopAvailable || w.busy}
            onChange={(value) => {
              setBrowser(value as Browser);
              setProfile(null);
              session.clearResult();
            }}
          />
          {browser && (
            <>
              <Select
                label={browser === 'safari' ? 'Cookie store' : 'Profile'}
                placeholder={
                  session.refreshing ? 'Finding profiles…' : 'Choose a profile'
                }
                value={available ? profile : null}
                data={
                  catalog?.profiles.map((candidate) => ({
                    value: candidate.id,
                    label: `${candidate.label} · ${candidate.detail}`,
                  })) ?? []
                }
                disabled={w.busy || session.refreshing}
                searchable
                allowDeselect={false}
                onChange={(value) => {
                  setProfile(value);
                  session.clearResult();
                }}
              />
              <Group justify="space-between">
                <Button
                  size="xs"
                  variant="subtle"
                  loading={session.refreshing}
                  disabled={w.busy}
                  onClick={() => {
                    session.clearResult();
                    void session.refresh(browser);
                  }}
                >
                  Refresh profiles
                </Button>
                <Button
                  disabled={
                    w.busy || !available || session.refreshing || selected
                  }
                  onClick={() => {
                    if (profile) session.attach(browser, profile);
                  }}
                >
                  {selected ? 'Using this session' : 'Use this session'}
                </Button>
              </Group>
              {catalog?.notice && (
                <Text size="xs" c="dimmed">
                  {catalog.notice}
                </Text>
              )}
              {profile && !available && !session.refreshing && (
                <Alert color="orange">
                  The selected profile is unavailable. Refresh the list and
                  choose your signed-in profile. Jacket won’t switch to another
                  account automatically.
                </Alert>
              )}
              <Accordion variant="default" key={`${browser}-${profile}`}>
                <Accordion.Item value="test">
                  <Accordion.Control disabled={session.testing}>
                    Test access (optional)
                  </Accordion.Control>
                  <Accordion.Panel>
                    <Stack gap="sm">
                      <TextInput
                        label="Link to test"
                        description="Checks access to the first item without downloading media."
                        placeholder="YouTube or YouTube Music link"
                        value={url}
                        disabled={w.busy}
                        onChange={(event) => {
                          setUrl(event.currentTarget.value);
                          session.clearResult();
                        }}
                      />
                      <Group>
                        <Button
                          variant="default"
                          loading={session.testing}
                          disabled={
                            w.busy ||
                            !available ||
                            session.refreshing ||
                            !url.trim()
                          }
                          onClick={() => {
                            if (profile)
                              void session.test(browser, profile, url.trim());
                          }}
                        >
                          Test access
                        </Button>
                        {session.testing && (
                          <Button
                            variant="subtle"
                            onClick={() => void session.cancelTest()}
                          >
                            Cancel test
                          </Button>
                        )}
                      </Group>
                      {result && (
                        <Alert
                          color={result.accessible ? 'green' : 'orange'}
                          title={
                            result.accessible
                              ? 'Access test passed'
                              : 'Access needs attention'
                          }
                          role="status"
                        >
                          <Text size="sm">{result.message}</Text>
                          <Text size="xs" mt="xs">
                            {result.guidance}
                          </Text>
                        </Alert>
                      )}
                    </Stack>
                  </Accordion.Panel>
                </Accordion.Item>
              </Accordion>
            </>
          )}
          {session.error && (
            <Alert color="red" role="alert">
              {session.error}
            </Alert>
          )}
          {(session.error || (result && !result.accessible)) && (
            <Text size="sm" c="dimmed">
              To reconnect, sign in to YouTube in this browser profile, then
              test access again.
            </Text>
          )}
          <Text size="sm" c="dimmed">
            Sessions stay on this computer. Your system may ask for permission
            to read them. Use a session only when needed; downloads can lead to
            account restrictions. Stopping session use leaves your browser
            signed in.
          </Text>
        </>
      )}
    </Stack>
  );
}
