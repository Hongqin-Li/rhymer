<h1 align="center">
    Rhymer<br/>
    <a href="../../actions"><img src="../../workflows/CI/badge.svg" alt="CI Status" style="max-width:100%;"></a>
    <a href="../../actions"><img src="../../workflows/Deploy/badge.svg" alt="Deploy Status" style="max-width:100%;"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License" style="max-width:100%;"></a>
</h1>

<p align="center">An open source backend-as-a-service library in Rust</p>

<br/><br/>

This is an API server module in Rust using [warp](https://github.com/seanmonstar/warp) and [MongoDB](https://github.com/mongodb/mongo-rust-driver), motivated by [Parse Server](https://parseplatform.org/). Still work-in-progress.

- [x] (Tested) User login/signup/update
- [x] (Tested) Object CRUD
- [x] (Tested) File upload/retrieve/delete, upload binary by application/x-www-form-urlencoded 
- [x] (Tested) Access control
- [x] (Tested) Hook(before/after save/destroy object, before/after save/delete file)
- [x] (Tested) Function
- [x] Github CI with `cargo test` and Github page with `cargo doc`
- [ ] User email
- [ ] TLS



## Getting Started

Make sure you have the following components installed.

- Rust toolchains
- MongoDB Server

Then add to your `Cargo.toml`

```toml
rhymer = { git = "https://github.com/Hongqin-Li/rhymer" }
```

See examples or [documentation](https://hongqin-li.github.io/rhymer/rhymer/index.html) for details.

### Configuration

The Rhymer server can be configured by the following options.

- `port` Port to listen on by the server.
- `secret` The Master key used to generate session tokens, **keep it as secret as possible**.
- `database_url` URL to MongoDB server including the database name, user name and password.
- `server_url` URL to this server, used to generate URL for uploaded files.
- `body_limit` Maximum number of bytes of body of request from client.



## API Documentation

### User

#### Signing up

Create a new user by providing `username`, `password` and some other data to be stored along with the newly created user. Note that the length of user name should be longer than or equal to 5 and it should only contains numbers `0-9`, alphabets `a-zA-Z` or hyphen `-`.

To sign up a new user, send a POST request to the server with body containing at least valid `username` and `password`. For example, to create a user with phone number:

```shell
curl -X POST -H "Content-Type: application/json" \
    -d '{"username":"foobar","password":"123456","phone":"123-456-7890"}' \
    http:localhost:8086/users
```

When the creation is successful, the HTTP response is of code `201 Created`  and body containing the user object such as

```json
{
   "username" : "foobar",
   "password" : "123456",
   "objectId" : "600136d7004ab8e200a7b06a",
   "createdAt" : "2021-01-15T06:31:51Z",
   "updatedAt" : "2021-01-15T06:31:51Z",
   "sessionToken" : "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiI2MDAxMzZkNzAwNGFiOGUyMDBhN2IwNmEiLCJleHAiOjE2MTA2OTMyMTEsImlkIjoiNjAwMTM2ZDcwMDRhYjhlMjAwYTdiMDZhIiwibmFtZSI6ImZvb2JhciJ9.gs0ye1EnZYGse_Wk6ekP4psj6teY2q4ac1sZRoYKpo4",
    "phone" : "123-456-7890",
}
```

If we are trying to sign up with user name registered before, we will get a response with code `409 Conflict`.

#### Logging in

Client can use `username` and `password` to log in. Server will query and verified matched user object in database, generating a JWT(JSON Web Token) encrypted by Master Key. The token can be used to verified quickly without querying database each time and will expire after 15 minutes.

For example, we can send a POST request with url-encoded `username` and `password` to log in:

```shell
curl -X GET "http:localhost:8086/login?username=$name&password=$pwd"
```

Note that the password embedded in the URL may be probed by listeners in the network iwhen transfered by HTTP protocol. A best-practice is to use HTTPS instead, which may be supported in the future. The JWT is inserted into `sesssionToken` field of returned user object.

#### Updating Users

User objects are allowed to be updated by verified users. In this way, users can update their user name, password or other arbitrary fields using a verified session token obtained by logging in. For safety reasons, update requests from client other than current user and master are rejected. Old fields not presented in the request body won't be removed.

For example, to update user identified by `id`, we can send a POST request with a `x-parse-session-token` header of JWT `token` such as

```shell
curl -X POST -H "Content-Type: application/json"
    -H "x-parse-session-token: $token" \
    -d '{"username":"foobar2","password":"1234567","other": "data"}' \
    http:localhost:8086/users/users/$id
```



### Object

Storing data through RESTful API is built around a JSON encoding of the object data. Data of object is schemaless by the nature of MongoDB, which means that we don't need to describe the structure of table ahead of time in RDBs like MySQL. Just pass key-value pairs to the backend and it will save it.

#### Object and Class

Each object belongs to some class. Same as name or password of user, name of classes that are allowed to be manipulated should only consists of numbers `0-9`, alphabets `a-zA-Z` and hyphen `-`. Classes with name that prefixed with underscore `_` are used internally by the server, such as `_User`. 

After creation, each object will generate an unique ID `objectId`, . Every future update operation on this object will modified its `updatedAt` field by the time when the operation is done. Thus, in most cases, each object has at least the following fields

```json
{
  "objectId" : "600062600096691c004951e0",
  "createdAt" : "2021-01-14T15:25:20Z",
  "updatedAt" : "2021-01-14T15:25:20Z",
  ...
}
```

`createdAt` and `updatedAt` are UTC timestamps in ISO 8601 format. `objectId` is a unique string that can be used to retrieve this object.

#### Creating Objects

The creation through RESTFul API is quite straightforwards, providing class in url path and data in body such as follows. For example, to save a song by Vitas

```shell
curl -X POST -H "Content-Type: application/json" \
  -d '{"name": "Dedication", "by": "Vitas", "released": "2003"}' \
  http://localhost:8086/classes/Song
```

This script will create an object in class `Song`

#### Retrieving Objects

Once the creation completes, we can send a GET request to the server with returned `objectId` to retrieve this object such as

```shell
curl -X GET "http://localhost:8086/classes/Song/$id"
```

Successful retrieving will return a response with code `200 Ok` and body containing the object data in JSON format. Else if object with such ID not exists, the server will response with code `404 Not Found`

#### Updating Objects

To change the fields of objected already exists, we can send a PUT request to the server routed by class name and object id. For example, if we want to modify the name of a movie in class `Movie`

```shell
curl -X POST -H "Content-Type: application/json" \
  -d '{"name": "The Fall"}' \
  http://localhost:8086/classes/Movie/$id
```

The response is an object before update.

#### Deleting Objects

To delete an object from the server, send a DELETE request with it's id such as

```shell
curl -X DELETE http://localhost:8086/classes/Movie/$id
```



### File

#### Uploading Files

To upload a file to the server, send a POST request with body of binary containing `X-Parse-Session-Token` header for user verification and `X-Parse-Application-Id` for file storage. Notice that we need to provide a `X-Parse-Application-Id`, which is used to distinguish our server when the file storage is shared among multiple applications. Only valid users with session token are legal to upload files, which can avoid attacks and help us keep trace of the uploaded files.

For example, to upload the Cargo.toml in current directory, run

```shell
appid=test-appid
curl -X POST -H 'Content-Type: application/x-www-form-urlencoded' \
    -H "X-Parse-Session-Token: $token" \
    -H "X-Parse-Application-Id: $appid" \
    --data-binary '@./Cargo.toml' \
    http://localhost:8086/files/Cargo.toml
```

After that the server will response with a `201 Created` and body json including `name` and `url` fields. The `url` can be used to directly access the file.

```json
{
   "name" : "600169b600e619d7000ac4f9-059ee62b-3f24-436f-a660-69b838ab715b-Cargo.toml",
   "url" : "http://localhost:8086/files/test-appid/600169b600e619d7000ac4f9-059ee62b-3f24-436f-a660-69b838ab715b-Cargo.toml"
}
```

#### Deleting Files

After uploading a file, the `name` field of the server response can be used to identified the file and remove it. This operation is only allowed with Master Key. For example, 

```shell
curl -X DELETE -H "X-Parse-Master-Key: $master_key" \
    http://localhost:8086/files/$appid/$name
```



### ACL

Access Control (ACL) together with hooks is the heart of Backend-as-a-Service. Instead of directly describe the business logic in code mixed with database operations, BaaS adopts another approach that separate them into ACL, hook functions and CRUD storage operations. CRUD storage operations have been fully implemented and provided by BaaS itself, while the ACL and hooks are application-specific and need to be given by BaaS users.

In Rhymer, each object has three kinds of access control level, `Invisiable`, `Read-only` and `Read-write` with respect to every users, default to read-write by all users. **ACL can only be modified when using this library in Rust**. Usually, we configure a `Acl`, setting per-user permissions by `set_invisiable(user_id)`, `set_readonly(user_id)`  and `set_writable(user_id)` and setting permission of other users not specified by `set_public_invisiable()`, `set_public_readonly()` and `set_public_writable`. Then use `Object::set_acl` function to apply the access control on objects. The ACL will be activated after saving the object successfully.

Support we have user id `uid` and request context `ctx`, and want to save a object of class `PrivateItem` private to the creator, we can code as follows.

```rust
let mut acl = Acl::new();
acl.set_public_invisiable();
acl.set_writable(uid);

let mut obj = ctx.object("PrivateItem");
obj.set_data(data);
obj.set_acl(acl);
obj.save().await?;
```

 

### Hook

A hook is some code snippet to execute before and after certain operations. In Rhymer, we can configure hooks to triggered before and after saving/deleting object and files.



### Function

Sometimes, we need to run some code that behaves differently from traditional CRUD operations, which is hard to implement in terms of hooks, we can use functions. After defining a function in Rust backend, we can trigger it by sending a POST request to the server with JSON body of key-value mappings as function arguments. Both key and value should be able to be deserialized into string.

For example, to trigger function `Add`, send a POST request with body of arguments such as

```shell
curl -X POST -H "Content-Type: application/json" \
  -d '{"arg1": "1", "arg2": "2"}' \
  http://localhost:8086/functions/Add
```

Alternatively, we can also GET requests with query string as key-value arguments to be more RESTFul when triggering stateless functions. The following behaves identical to the script above.

```shell
curl -X GET "http:localhost:8086/functions/Add?arg1=1&arg2=2"
```



## Development

### Initialize MongoDB

If you want to try on MongoDB docker, just run `sh scripts/initdb-docker.sh` and skip this section.

Otherwise, you need to create a user with username, password and a database of name "rhymer-test".

```javascript
// Login your admin user so that we can create new user.
use admin
db.auth(xxx, xxx);

use rhymer-test
db.createUser({
  user: 'rhymer-test',
  pwd: 'rhymer-test',
  roles: ['readWrite']
});
```

Then execute the initialization script `scripts/init-db.js` on MongoDB. For example, when using official docker image, start it and run `docker exec -i mongo mongo < scripts/init-db.js`.

## Known Issues

- `sudo apt install libssl-dev` when build process failed.

- [Error convert string Json to Document using Serde.](https://github.com/mongodb/bson-rust/issues/189)
