import type { ProviderAccount, ProviderProfile } from "@/lib/types"

export function flattenProviderProfiles(
  providerAccounts: ProviderAccount[],
): ProviderProfile[] {
  const profiles: ProviderProfile[] = []

  for (const provider of providerAccounts) {
    for (const model of provider.models) {
      profiles.push({
        id: model.id,
        provider_id: provider.id,
        model_name: model.name || model.model,
        name: provider.name,
        base_url: provider.base_url,
        api_key: provider.api_key,
        model: model.model,
        created_at: model.created_at,
        updated_at: model.updated_at,
      })
    }
  }

  profiles.sort((left, right) => left.created_at.localeCompare(right.created_at))
  return profiles
}
