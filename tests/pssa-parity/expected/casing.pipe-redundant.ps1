ForEach ($x IN $y) {
    IF ($x -EQ 1) { BREAK }
    ELSEIF ($x -NE 2) { CONTINUE }
}
TRY { get-childitem -path C:\ -recurse } CATCH { }
FUNCTION Test-Casing {
    PARAM([string] $Value)
    RETURN $Value
}
$r = 'a' -REPLACE 'b' -SPLIT 'c' -JOIN 'd'
$b = $x -BAND $y -BOR $z
WHILE ($FALSE) { }
DO { } WHILE ($FALSE)
write-output 'lower cmdlet'
GET-PROCESS | where-object { $_ }
