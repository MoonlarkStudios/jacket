import { Button, TextInput, Tooltip } from '@mantine/core';
import {
  IconArrowRight,
  IconLink,
  IconLoader2,
  IconX,
} from '@tabler/icons-react';
import { type RefObject, useId } from 'react';
import { pastedSourceUrl } from '../preferences';

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
  return (
    <form
      className="source-form"
      onSubmit={(event) => {
        event.preventDefault();
        if (!disabled && !loading && value.trim()) submit(value.trim());
      }}
    >
      <label className="source-label" htmlFor={inputId}>
        YouTube or YouTube Music link
      </label>
      <div className="source-entry">
        <TextInput
          id={inputId}
          ref={inputRef}
          placeholder="Paste an album or playlist link"
          aria-label="YouTube or YouTube Music link"
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
              change(source);
              submit(source);
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
    </form>
  );
}
