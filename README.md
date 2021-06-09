## ws_router

A simple server-side websocket router. Allows for devices on different private networks to communicate directly with each other, over whatever server this is deployed on.

### How it works
First, you'll make an HTTP GET to `http(s)://server:port/register` with the following URL Query parameters (all of which are required):
| Parameter | Type | Description |
| - | - | - |
| `key` | String | The key that websocket connections must use when trying to connect to this registration. |
| `host_key` | String | The key that someone will need to use to remove this registration while users are still connected to it. |
| `reg_type` | String | Must be either `hostclient` or `lobby`. If `hostclient`, all connections will need to either act as a host or a client, and each connection's messages will only be passed to connections of the other type. If `lobby`, all connections' messages will be sent to all other connections. |

The response from this request will be a random string (a UUID), 32 digits long (the length of this string may change in later versions of ws_router). To connect to the registration that was just created with this most recent request, you'll connect via to a websocket via `ws(s)://server:port/connect` with the following URL Query parameters:

| Parameter | Required? |Type | Description |
| - | - | - | - |
| `id` | Yes | String | The UUID that was sent back from the registration request described in the last step.
| `key` | Yes | String | The `key` that was sent along with the registration request for the id specified by `id`. |
| `sock_type` | If the `reg_type` is `hostclient` | String | If the `reg_type` for the accompanying registration was `hostclient`, this must either be `host` or `client` (depending on whether the device that is trying to connect is acting as a host or a client). If the `reg_type` is `lobby`, this parameter is not necessary. |

A registration is automatically removed from the internal registration store as soon as it has been connected to at least once and there are no longer any devices connected to it. It can also be manually removed (and all of its connections disconnected once they try to send another message) by sending an HTTP GET request to `http(s)://server:port/remove` with the following URL query parameters (all of which are required):

| Parameter | Type | Description |
| - | - | - |
| `id` | String | The UUID of the registration that you would like to remove |
| `key` | String | The key that was sent along with the registration request for the id specified by `id`. |
| `host_key` | String | The host_key that was sent along with the registration request for the id specified by `id`. |

Once a device has been connected to a certain registration, it can keep on communicating through that connection and the registration that it is connected to, as long as the registration has not been removed.

### Building
Just as with any rust program &mdash;
```sh
git clone https://github.com/iandwelker/ws_router.git
cd ws_router
cargo build --release
```
Your binary will be in target/release :)

__To generate certificates__ to be used with this, run:
```
openssl genrsa -out key.rsa 3072
openssl req -new -x509 -key key.rsa -out cert.pem -days 360
```
and fill out all the forms it asks you about. You can leave all of them blank besides the common name, which needs to have a value.

### Contributing
If you have any questions or suggestions or features to add, feel free to file an issue or a PR!
