# hpos-api-rust

An API for interacting with holochain services running on HPOS.

## Service

Binary requires the following env vars for running and being fully aware of its environment:
```
HOLOCHAIN_DEFAULT_PASSWORD
DEVICE_SEED_DEFAULT_PASSWORD
HPOS_CONFIG_PATH - set by hpos-init
CORE_HAPP_FILE
LAIR_WORKING_DIR
DEV_UID_OVERRIDE
SL_COLLECTOR_PUB_KEY
HOST_PUBKEY_PATH *(Required only in non-test envs)*
IS_TEST_ENV *(Required only to be set as true in test env)* 
```

## Authentication

This API is relying on an authentication mechanism [hp-admin-crypto](https://github.com/Holo-Host/hp-admin-crypto).

## API

This API is mounted on HPOS at v2 path of API, so all the calls should be of a format `/api/v2/<path>`, e.g. to get all hosted happs with usage calculated over last 7 days you would call `/api/v2/hosted_happs/?usage_interval=604800`.

## Integration Tests

```
RUST_LOG=hpos-api-rust=trace,integration=trace cargo test -- --nocapture --test-threads=1
```

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

#### GET `/hosted_happs/<id>/logs?<days>`
```
Record {
    /// The signed action for this record
    pub signed_action: SignedActionHashed,
    /// If there is an entry associated with this action it will be here.
    /// If not, there will be an enum variant explaining the reason.
    pub entry: `Hidden` | `NA` | `NotStored` | `Present`: <Entry>,
}
```

#### POST `/zome_call`
Makes a zome call with parameters specified in a request to holochain instance running on HPOS. Call is signed as an agent from HPOS config (same as the one used for interaction with holochain via other endpoints of this API).
```
ZomeCallRequest {
    app_id: String,
    role_id: String,
    zome_name: String,
    fn_name: String,
    payload: Object, // Object reperesenting a zome call payload
}
```

200 OK
returns response `application/octet-stream` - a byte payload exactly as returned by holochain. It is up to the caller to use msgpack to decode this message and parse content.
