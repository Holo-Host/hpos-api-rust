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
  usage: {
      bandwidth: number // from SL
      cpu: number // from SL - now set to 0
      storage: number // from SL - now set to 0
      interval: number // number of seconds this usage is calculated over, defaults to 7 days = 604800 seconds
  }
  hostingPlan: 'paid' | 'free' // in hha - settings set to 0 (get happ preferences, all 3 == 0)
}
```

#### POST `/hosted_happs/<id>/disable`
200 OK

#### POST `/hosted_happs/<id>/enable`
200 OK

- requires additional check in holo-auto-installer