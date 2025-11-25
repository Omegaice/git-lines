---
description: Validate a design using plain english
---

Use the Task tool with subagent_type "general-purpose", model "opus" and the following prompt, replacing `<code>` with $ARGUMENTS:

```markdown
You are validating a software design by translating it to plain English, then analyzing if the translation reveals design issues.

**Domain Context**: This code is for domain experts. Standard domain terminology (terms from domain documentation, literature, or common usage) is APPROPRIATE and EXPECTED. Only flag programming/implementation jargon.

**Critical distinction**:
- ✅ Domain-standard terms (e.g., "transaction" in databases, "hunk" in Git diffs) = GOOD, use freely
- ❌ Programming patterns/convenience terms (e.g., "manager", "helper", "handler", "factory") = BAD, signals poor abstraction

<code>

## What This Validation Tests

Translation to plain English reveals design quality:
- ✅ Natural, flowing explanation = Code structure matches domain concepts
- ❌ Awkward, complex explanation = Poor abstractions or wrong model
- ❌ Requires programming concepts to explain = Implementation details leaking into domain model
- ✅ Uses domain-standard terminology = Expected and appropriate

You're testing whether the CODE STRUCTURE reflects how domain experts think about the problem.

## Phase 1: Plain English Description

Describe what each piece represents in the real-world domain.

**Rules**:
1. Use natural language with standard domain terminology
2. Avoid PROGRAMMING terms: struct, field, enum, variant, type, class, interface, method, function, etc.
3. DO use domain-standard terms when appropriate - that's expected
4. Note which domain-specific terms you use (helps distinguish domain vs implementation language later)
5. Focus on WHAT things represent in the domain, not HOW they're implemented
6. Pay attention to when explanation feels awkward or requires qualifications - that's a design smell

Start your description with: "This represents..."

## Phase 2: Critical Analysis

Now critically evaluate the design based on your description. Be maximally critical - your job is to find problems, not validate existing work.

Do not read any files unless specifically instructed to.

Add each of the following red flags to your todo list using TodoWrite:

1. **Complexity mismatch**: Is the explanation more complex than the domain concept it represents?
2. **Implementation leakage**: Did you need programming concepts (not domain concepts) to explain the design? This means implementation details are leaking into the domain model.
3. **Name mismatch**: Do the code names match the natural language you used? If not, the naming is wrong. (Exception: domain-standard terms matching code names is good)
4. **Missing concepts**: Are there domain concepts you mentioned that aren't explicitly modeled in the code?
5. **Redundancy**: Did you repeat similar explanations? That suggests a missing abstraction.
6. **Indirection**: Did you need multiple steps to explain something simple in the domain?
7. **Struggle points**: What was hard to explain? Why was it hard?
8. **Freeform**: Were there any other problems that didn't fit the categories above?

For each item on your todo, think hard about:
- What the issue is (be specific)
- Why it matters (real consequences)
- How it could be improved (concrete suggestions)

## Final Response Format

After completing your analysis, format your response EXACTLY as shown below. The user cannot see your intermediate work - they only see your final response. You must copy ALL the detailed analysis you wrote above into this format.

**Template**:

---
## Domain Description

[Paste your complete Phase 1 description here]

## Domain-Specific Terms Used

[List the domain terms you used and note that these are appropriate domain vocabulary]

## Design Issues

### 1. [Red Flag Name]

**Issue**: [Complete detailed explanation of what's wrong - copy from your analysis above]

**Why it matters**: [Complete explanation of impact - copy from your analysis above]

**How to improve**: [Complete concrete suggestions - copy from your analysis above]

[Repeat for each red flag you identified]

---

If you found no issues, state: "No design issues found. The model maps cleanly to domain concepts."

CRITICAL: You must include COMPLETE TEXT from all your analyses above. The user has NOT seen your intermediate work. Do not summarize - paste the full detailed explanations you already wrote.
```
