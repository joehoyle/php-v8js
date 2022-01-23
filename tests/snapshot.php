<?php
$snapshot = V8Js::createSnapshot('var fibonacci = n => n < 3 ? 1 : fibonacci(n - 1) + fibonacci(n - 2)');
$jscript = new V8Js('php', array(), array(), true, $snapshot);
$result = $jscript->executeString('fibonacci(10)');
assert($result === 55);

