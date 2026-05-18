<script>
  import { useAppState } from '../lib/context.js'
  import TrimPanel from './TrimPanel.svelte'
  import ResizePanel from './ResizePanel.svelte'
  import ExportPanel from './ExportPanel.svelte'
  import RemapPanel from './RemapPanel.svelte'
  import ConvertColorSpacePanel from './ConvertColorSpacePanel.svelte'
  import FlattenSpotsPanel from './FlattenSpotsPanel.svelte'
  import AddTrimBoxPanel from './AddTrimBoxPanel.svelte'
  import SplitPagesPanel from './SplitPagesPanel.svelte'
  import StitchPagesPanel from './StitchPagesPanel.svelte'
  import ExtractPagesPanel from './ExtractPagesPanel.svelte'
  import OutputPanel from './OutputPanel.svelte'
  const app = useAppState()
</script>

<div class="params-panel">
  {#if app.activeAction === 'trim'}
    <TrimPanel />
  {:else if app.activeAction === 'resize'}
    <ResizePanel />
  {:else if app.activeAction === 'export'}
    <ExportPanel />
  {:else if app.activeAction === 'remap'}
    <RemapPanel />
  {:else if app.activeAction === 'colorspace'}
    <ConvertColorSpacePanel />
  {:else if app.activeAction === 'spots'}
    <FlattenSpotsPanel />
  {:else if app.activeAction === 'addtrimbox'}
    <AddTrimBoxPanel />
  {:else if app.activeAction === 'splitpages'}
    <SplitPagesPanel />
  {:else if app.activeAction === 'stitchpages'}
    <StitchPagesPanel />
  {:else if app.activeAction === 'extractpages'}
    <ExtractPagesPanel />
  {:else if app.activeAction === 'output'}
    <OutputPanel />
  {/if}
</div>

<style>
  .params-panel {
    flex: 1;
    overflow-y: auto;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
</style>
