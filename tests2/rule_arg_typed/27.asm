#ruledef test
{
    ld {x: s8} => 0x55 @ x
}

ld 128 ; error: out of range