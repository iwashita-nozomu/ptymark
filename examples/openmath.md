# OpenMath preview example

The block below represents `x + 1 = 2` using OpenMath 2 XML.

```openmath
<OMOBJ xmlns="http://www.openmath.org/OpenMath" version="2.0">
  <OMA>
    <OMS cd="relation1" name="eq"/>
    <OMA>
      <OMS cd="arith1" name="plus"/>
      <OMV name="x"/>
      <OMI>1</OMI>
    </OMA>
    <OMI>2</OMI>
  </OMA>
</OMOBJ>
```

The next block uses a project-specific Content Dictionary. Ptymark keeps the symbol visible through its generic `cd.name` presentation without downloading a dictionary.

```openmath
<OMOBJ xmlns="http://www.openmath.org/OpenMath" version="2.0">
  <OMA>
    <OMS cd="research1" name="wave_operator"/>
    <OMV name="psi"/>
  </OMA>
</OMOBJ>
```
