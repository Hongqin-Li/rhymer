cmd=$1
host="http://localhost:8086"
curl="curl"

if [ "$cmd" = "signup" ]; then
    $curl -X POST -H "Content-Type: application/json" \
         -d '{"username":"cooldude6","password":"p_n7!-e8","phone":"415-392-0202"}' \
         $host/users

elif [ "$cmd" = "login" ]; then
    $curl -X GET "$host/login?username=foo&password=bar"

elif [ "$cmd" = "create" ]; then
    $curl -X POST -H "Content-Type: application/json" \
         -d '{"score":"1337","playerName":"Sean Plott","cheatMode":"false"}' \
         "$host/classes/my-class"

elif [ "$cmd" = "retrieve" ]; then
    # Passing empty id to retrieve all
    $curl -X GET "$host/classes/my-class/$2"

elif [ "$cmd" = "retrieve-by-filter" ]; then
    $curl -X GET -G "$host/classes/my-class" -d $2

elif [ "$cmd" = "update" ]; then
    $curl -X PUT "$host/classes/my-class/$2" -H "Content-Type: application/json" -d '{"newField": "newValue"}'
    
elif [ "$cmd" = "delete" ]; then
    $curl -X DELETE "$host/classes/my-class/$2"

else
    echo "invalid command"
    exit 1
fi
