# hpos-api-rust

An API for interacting with holochain services running on HPOS.

## Authentication

This API is relying on an authentication mechanism [hp-admin-crypto](https://github.com/Holo-Host/hp-admin-crypto).

## API

This API is mounted at v2 path, so all calls should be of a format `/v2/<path>`

### Endpoints

#### GET `/hosted_happs/?quantity=<quantity>&usage_interval=<usage_interval>`
- `quantity: u32` - max number of happs to return ordered by earnings within last 7 days, if omitted all happs will be returned
- `usage_interval: u32` - (required) include statistics from last `<usage_interval>` seconds
```
Vec<HappDetails>
```

#### GET `/hosted_happs/<id>`
```
HappDetails {
  id: string
  name: string
  description: string
  categories: string[]
  enabled: boolean
  isPaused: boolean
  sourceChains: number | null     // null when calculation has failed
  daysHosted: number | null       // null when calculation has failed
  earnings: {
      total: number
      last7Days: number
      averageWeekly: number
  } | null                        // null when calculation has failed
  usage: {
      bandwidth: number
      cpu: number
      storage: number
      interval: number            // number of seconds this usage is calculated over, defaults to 7 days = 604800 seconds
  } | null                        // null when calculation has failed
  hostingPlan: 'paid' | 'free' | null // free if all 3 hosting prefs are set to 0 - when calculation has failed
}
```

#### POST `/hosted_happs/<id>/disable`
200 OK

#### POST `/hosted_happs/<id>/enable`
200 OK

- requires additional check in holo-auto-installer