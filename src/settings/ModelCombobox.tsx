import { useEffect, useId, useMemo, useRef, useState } from "react";

interface Props {
  id?: string;
  value: string;
  options: string[];
  placeholder?: string;
  disabled?: boolean;
  onChange: (value: string) => void;
}

export function filterModelOptions(options: string[], query: string): string[] {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return options;
  return options.filter((option) => option.toLocaleLowerCase().includes(normalized));
}

export function ModelCombobox({
  id,
  value,
  options,
  placeholder,
  disabled = false,
  onChange,
}: Props) {
  const generatedId = useId();
  const listId = `${id ?? generatedId}-options`;
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [filtering, setFiltering] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const filteredOptions = useMemo(
    () => filterModelOptions(options, filtering ? value : ""),
    [filtering, options, value],
  );

  useEffect(() => {
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
        setFiltering(false);
      }
    };
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, []);

  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  const showOptions = () => {
    if (!disabled && options.length) {
      setFiltering(false);
      setActiveIndex(0);
      setOpen(true);
    }
  };

  const selectOption = (option: string) => {
    onChange(option);
    setOpen(false);
    setFiltering(false);
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      setOpen(false);
      setFiltering(false);
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (!open) {
        showOptions();
      } else if (filteredOptions.length) {
        setActiveIndex((index) => Math.min(index + 1, filteredOptions.length - 1));
      }
      return;
    }
    if (event.key === "ArrowUp" && open && filteredOptions.length) {
      event.preventDefault();
      setActiveIndex((index) => Math.max(index - 1, 0));
      return;
    }
    if (event.key === "Enter" && open && filteredOptions[activeIndex]) {
      event.preventDefault();
      selectOption(filteredOptions[activeIndex]);
    }
  };

  return (
    <div className="settings-model-combobox" ref={rootRef}>
      <input
        id={id}
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={
          open && filteredOptions[activeIndex] ? `${listId}-${activeIndex}` : undefined
        }
        onFocus={showOptions}
        onClick={showOptions}
        onChange={(event) => {
          const nextValue = event.target.value;
          onChange(nextValue);
          setFiltering(true);
          setActiveIndex(0);
          setOpen(options.length > 0);
        }}
        onKeyDown={handleKeyDown}
      />
      {open && (
        <div className="settings-model-options" id={listId} role="listbox">
          {filteredOptions.length ? (
            filteredOptions.map((option, index) => (
              <button
                type="button"
                id={`${listId}-${index}`}
                role="option"
                aria-selected={option === value}
                className={index === activeIndex ? "is-active" : undefined}
                key={option}
                onMouseDown={(event) => event.preventDefault()}
                onMouseEnter={() => setActiveIndex(index)}
                onClick={() => selectOption(option)}
              >
                {option}
              </button>
            ))
          ) : (
            <span className="settings-model-options__empty">无匹配模型，可继续手动输入</span>
          )}
        </div>
      )}
    </div>
  );
}
