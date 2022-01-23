<?php

$v8js = new V8Js;
$result = $v8js->executeString( '5 + 5' );
assert( $result === 10 );
