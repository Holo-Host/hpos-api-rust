# hpos-api-rust

An API for interacting with holochain services running on HPOS.

## Authentication

This API is relying on an authentication mechanism [hp-admin-crypto](https://github.com/Holo-Host/hp-admin-crypto).

## API

This API is mounted at v2 path, so all calls should be of a format `/v2/<path>`

### Endpoints

#### GET `/hosted_happs/?quantity=<quantity>&usage_interval=<usage_interval>`
- ~~`quantity` - max number of happs to return, if omited all happs will be returned [TODO: is it even used? sounds like a half-baked pagination attempt]~~
- `usage_interval` - (required) include statistics from last `<usage_interval>` days
```
Vec<HappDetails>
```

#### GET `/hosted_happs/<id>`
```
HappDetails {
  id: string // from hha
  name: string // from hha
  description: string // from hha
  categories: string[] // from hha
  enabled: boolean // from hha
  isPaused: boolean // from hha
  sourceChains: number // counting instances of a given happ by it's name (id)
  daysHosted: number // timestamp on a link of enable happ
  earnings: {
      total: number    // From holofuel
      last7Days: number    // From holofuel
      averageWeekly: number    // From holofuel
  }
  last7DaysUsage: {
      bandwidth: number // from SL
      cpu: number // from SL - now set to 0
      storage: number // from SL - now set to 0
  }
  hostingPlan: 'paid' | 'free' // in hha - settings set to 0 (get happ preferences, all 3 == 0)
}
```

#### POST `/hosted_happs/<id>/disable`
200 OK

#### POST `/hosted_happs/<id>/enable`
200 OK

- requires additional check in holo-auto-installer