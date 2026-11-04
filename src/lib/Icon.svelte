<script lang="ts">
  import type { View } from "./view";

  let { name, size = 18 }: { name: View; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.4"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "ai"}
    <path d="M8 2l1.6 3.9L13.5 7.5 9.6 9.1 8 13 6.4 9.1 2.5 7.5 6.4 5.9z" />
  {:else if name === "agents"}
    <rect x="2" y="3" width="12" height="10" rx="2" /><path d="M5 7l2 1.5L5 10M8.5 10H11" />
  {:else if name === "github"}
    <circle cx="5" cy="4" r="1.5" /><circle cx="5" cy="12" r="1.5" /><circle cx="11" cy="6" r="1.5" />
    <path d="M5 5.5v5M11 7.5c0 2-6 1.5-6 3" />
  {:else if name === "tasks"}
    <path d="m2 4 1 1 2-2M7 4h7m-12 6 1 1 2-2M7 10h7" />
  {:else if name === "network"}
    <path d="M2 6.5a9 9 0 0112 0M4 9a6 6 0 018 0M6 11.5a3 3 0 014 0" /><circle cx="8" cy="13.5" r=".6" />
  {:else if name === "system"}
    <rect x="4" y="4" width="8" height="8" rx="1" />
    <path d="M6 2v2M10 2v2M6 12v2M10 12v2M2 6h2M2 10h2M12 6h2M12 10h2" />
  {:else}
    <path
      d="M6.92 1.49L9.08 1.49L9.21 3.15L10.58 3.72L11.84 2.63L13.37 4.16L12.28 5.42L12.85 6.79L14.51 6.92L14.51 9.08L12.85 9.21L12.28 10.58L13.37 11.84L11.84 13.37L10.58 12.28L9.21 12.85L9.08 14.51L6.92 14.51L6.79 12.85L5.42 12.28L4.16 13.37L2.63 11.84L3.72 10.58L3.15 9.21L1.49 9.08L1.49 6.92L3.15 6.79L3.72 5.42L2.63 4.16L4.16 2.63L5.42 3.72L6.79 3.15Z"
    />
    <circle cx="8" cy="8" r="2" />
  {/if}
</svg>
