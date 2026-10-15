<script lang="ts">
  import { errorKey } from '../errors';
  import { t } from '../i18n';

  let { error }: { error: unknown } = $props();
</script>

<p class="error" role="alert" data-testid="error">{$t('common.error')}: {$t(errorKey(error))}</p>
