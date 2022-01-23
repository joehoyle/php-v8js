<?php

var_dump(extension_loaded('v8js'));
if (!extension_loaded('v8js')) {
	die("skip");
}
