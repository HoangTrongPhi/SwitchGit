import { cn } from "@abyss/ui";

/** An on/off switch. `label` names it for screen readers; the visible text sits next to it. */
export function Switch({ checked, onChange, label, disabled, size = "md" }: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
  size?: "sm" | "md";
}) {
  const sm = size === "sm";
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative shrink-0 rounded-full ring-1 transition-colors ring-inset focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:opacity-50",
        sm ? "h-4 w-7" : "h-[22px] w-10",
        checked ? "bg-emerald ring-emerald" : "bg-tint/10 ring-edge-strong",
      )}
    >
      <span
        className={cn(
          "absolute top-1/2 -translate-y-1/2 rounded-full bg-white shadow-sm transition-[left]",
          sm ? "size-3" : "size-[18px]",
          checked ? (sm ? "left-[14px]" : "left-5") : "left-0.5",
        )}
      />
    </button>
  );
}
