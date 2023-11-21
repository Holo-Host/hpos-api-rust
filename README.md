# hpos-api-rust

An API for interacting with holochain services running on HPOS.

## Service

Binary requires the following env vars for running and being fully aware of its environment:
```
HOLOCHAIN_DEFAULT_PASSWORD
DEVICE_SEED_DEFAULT_PASSWORD
HPOS_CONFIG_PATH - set by hpos-init
CORE_HAPP_FILE
HOLOCHAIN_WORKING_DIR
DEV_UID_OVERRIDE
```

## Authentication

This API is relying on an authentication mechanism [hp-admin-crypto](https://github.com/Holo-Host/hp-admin-crypto).

## API

This API is mounted on HPOS at v2 path of API, so all the calls should be of a format `/api/v2/<path>`, e.g. to get all hosted happs with usage calculated over last 7 days you would call `/api/v2/hosted_happs/?usage_interval=604800`.

### Endpoints

#### GET `/hosted_happs/?quantity=<quantity>&usage_interval=<usage_interval>`
- `quantity: u32` - max number of happs to return ordered by earnings within last 7 days, if omitted all happs will be returned
- `usage_interval: u32` - (required) include statistics from last `<usage_interval>` seconds
```
Vec<HappDetails>
```

#### GET `/hosted_happs/<id>?usage_interval=<usage_interval>`
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
      disk_usage: number
  } | null                        // null when calculation has failed
  hostingPlan: 'paid' | 'free' | null // free if all 3 hosting prefs are set to 0 - when calculation has failed
}
```

#### POST `/hosted_happs/<id>/disable`
200 OK

#### POST `/hosted_happs/<id>/enable`
200 OK

- requires additional check in holo-auto-installer

#### GET `/hosted_happs/get_default_preferences`
```
HappPreferences {
    timestamp: Timestamp,
    maxFuelBeforeInvoice: Fuel, 
    priceCompute: Fuel, // 0 for free hosting plans
    priceStorage: Fuel, // 0 for free hosting plans
    priceBandwidth: Fuel, // 0 for free hosting plans
    maxTimeBeforeInvoice: Duration,
}
```