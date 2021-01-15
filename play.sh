#!/usr/bin/env bash

cmd=$1
host="http://localhost:8086"
curl="curl"

# User
if [ "$cmd" = "signup" ]; then
    username=$2
    password=$3

    if [ -z $username ] || [ -z $password ]; then
        echo "usage: sh play.sh signup USERNAME PASSWORD"
        exit 1
    fi

    $curl -X POST -H "Content-Type: application/json" \
        -d '{"username": "'$username'", "password": "'$password'",  "phone": "123-456-7890"}' \
        $host/users

elif [ "$cmd" = "login" ]; then
    username=$2
    password=$3

    if [ -z $username ] || [ -z $password ]; then
        echo "usage: sh play.sh login USERNAME PASSWORD"
        exit 1
    fi

    $curl -X GET "$host/login?username=$username&password=$password"

elif [ "$cmd" = "update" ]; then
    id=$2
    token=$3
    $curl -X POST -H "Content-Type: application/json" \
        -H "x-parse-session-token: $token" \
        -d '{"username":"foobar2","password":"1234567","other": "data"}' \
        $host/users/$id

# Object
elif [ "$cmd" = "create" ]; then
    $curl -X POST -H "Content-Type: application/json" \
        -d '{"name": "A", "course": "Artificial Intelligence", "projects": 4}' \
        "$host/classes/Movie"

elif [ "$cmd" = "retrieve" ]; then
    # Passing empty id to retrieve all
    $curl -X GET "$host/classes/my-class/$2"

elif [ "$cmd" = "retrieve-by-filter" ]; then
    $curl -X GET -G "$host/classes/my-class" -d $2

elif [ "$cmd" = "update" ]; then
    $curl -X PUT "$host/classes/Movie/$2" -H "Content-Type: application/json" -d '{"name": "The Fall"}'
    
elif [ "$cmd" = "delete" ]; then
    $curl -X DELETE "$host/classes/Movie/$2"

# File
elif [ "$cmd" = "upload" ]; then
    token=$2
    $curl -X POST -H 'content-type: application/x-www-form-urlencoded' \
        -H "X-Parse-Session-Token: $token" \
        -H "X-Parse-Application-Id: test-appid" \
        --data-binary '@./Cargo.toml' \
        "$host/files/Cargo.toml"

elif [ "$cmd" = "download" ]; then
    $curl -X GET \
        "$host/files/$2"

elif [ "$cmd" = "delete-file" ]; then
    name=$2
    appid=$3
    master_key=$4

    if [ -z $name ] || [ -z "$appid" ]|| [ -z "$master_key" ]; then
        echo "usage: sh play.sh delete-file NAME APPID MASTER_KEY"
        exit 1
    fi

    $curl -X DELETE -H "X-Parse-Master-Key: $master_key" $host/files/$appid/$name

else
    echo "invalid command"
    exit 1
fi
