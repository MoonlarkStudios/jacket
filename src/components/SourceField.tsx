import { Button, SegmentedControl, TextInput, Tooltip } from '@mantine/core';
import {
  IconArrowRight,
  IconLink,
  IconLoader2,
  IconX,
} from '@tabler/icons-react';
import { type RefObject, useId, useState } from 'react';
import { pastedSourceUrl } from '../preferences';
import { type SourceIntent, sourceForIntent, sourceLink } from '../sourceLinks';
import './SourceField.css';

export function SourceField({
  value,
  change,
  submit,
  disabled = false,
  loading = false,
  inputRef,
  cancel,
}: {
  value: string;
  change: (value: string) => void;
  submit: (source: string) => void;
  disabled?: boolean;
  loading?: boolean;
  inputRef: RefObject<HTMLInputElement | null>;
  cancel?: () => void;
}) {
  const inputId = useId();
  const scopeId = useId();
  const [choice, setChoice] = useState<{
    source: string;
    intent: SourceIntent;
  } | null>(null);
  const link = sourceLink(value);
  const ambiguous = !!link?.songUrl && !!link.playlistUrl;
  const intent = choice?.source === value ? choice.intent : 'song';
  const submitSource = () => {
    if (!disabled && !loading && value.trim())
      submit(sourceForIntent(value, intent) || value.trim());
  };
  return (
    <form
      className="source-form"
      onSubmit={(event) => {
        event.preventDefault();
        submitSource();
      }}
    >
      <label className="source-label" htmlFor={inputId}>
        YouTube or YouTube Music link
      </label>
      <div className="source-entry">
        <TextInput
          id={inputId}
          ref={inputRef}
          placeholder="Paste a song, album, or playlist link"
          aria-label="YouTube or YouTube Music link"
          aria-describedby={ambiguous ? scopeId : undefined}
          leftSection={
            loading ? (
              <IconLoader2 size={18} className="spinner" aria-hidden="true" />
            ) : (
              <IconLink size={18} stroke={1.6} />
            )
          }
          value={value}
          onChange={(event) => change(event.currentTarget.value)}
          onPaste={(event) => {
            const source = pastedSourceUrl(event.clipboardData.getData('text'));
            if (source && !disabled && !loading) {
              event.preventDefault();
              setChoice(null);
              change(source);
              const pasted = sourceLink(source);
              if (!(pasted?.songUrl && pasted.playlistUrl))
                submit(sourceForIntent(source) || source);
            }
          }}
          disabled={disabled || loading}
          variant="unstyled"
          size="md"
        />
        <Tooltip
          label={loading && cancel ? 'Cancel lookup' : 'Find tracks · Enter'}
        >
          <Button
            type={loading && cancel ? 'button' : 'submit'}
            aria-label={loading && cancel ? 'Cancel lookup' : 'Find tracks'}
            size="md"
            loading={loading && !cancel}
            disabled={loading && cancel ? false : disabled || !value.trim()}
            onClick={loading && cancel ? cancel : undefined}
            rightSection={
              loading && cancel ? (
                <IconX size={18} />
              ) : (
                <IconArrowRight size={19} />
              )
            }
          >
            {loading && cancel ? 'Cancel' : 'Find tracks'}
          </Button>
        </Tooltip>
      </div>
      {ambiguous && (
        <div className="source-scope">
          <span className="source-scope-copy" id={scopeId}>
            This song is part of a playlist.
          </span>
          <SegmentedControl
            aria-label="What to download from this link"
            size="xs"
            value={intent}
            disabled={disabled || loading}
            onChange={(next) =>
              setChoice({ source: value, intent: next as SourceIntent })
            }
            data={[
              { value: 'song', label: 'This song' },
              { value: 'playlist', label: 'Entire playlist' },
            ]}
          />
        </div>
      )}
    </form>
  );
}
