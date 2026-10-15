import { describe, expect, it } from 'vitest';
import { verificationSteps, runVerification } from '../scripts/verify.mjs';

describe('project verification sequence', () => {
  it('runs standard checks and includes native smoke and Rust audit only when requested', () => {
    expect(verificationSteps().map(step => step.name)).toEqual([
      'frontend-tests', 'frontend-check', 'frontend-build', 'rust-tests', 'rust-check', 'npm-audit',
    ]);
    expect(verificationSteps({ smoke: true, auditRust: true }).slice(-2).map(step => step.name))
      .toEqual(['bundled-smoke', 'rust-audit']);
  });
  it('stops on the first failed command and retains its exit code', () => {
    const calls = [];
    const report = runVerification(verificationSteps(), step => {
      calls.push(step.name);
      return { status: step.name === 'frontend-check' ? 7 : 0 };
    });
    expect(calls).toEqual(['frontend-tests', 'frontend-check']);
    expect(report).toMatchObject({ outcome: 'failed', exitCode: 7 });
    expect(report.steps[1]).toMatchObject({ name: 'frontend-check', outcome: 'failed', exitCode: 7 });
  });
  it('fails when a command cannot launch or throws and never reports success', () => {
    const steps = verificationSteps();
    expect(runVerification(steps, () => ({ status: null, error: new Error('cannot launch') })))
      .toMatchObject({ outcome: 'failed', exitCode: 1 });
    expect(runVerification(steps, () => { throw new Error('runner failed'); }).steps[0].error).toBe('runner failed');
  });
  it('reports success only after every step exits successfully', () => {
    const report = runVerification(verificationSteps(), () => ({ status: 0 }));
    expect(report.outcome).toBe('passed');
    expect(report.steps).toHaveLength(6);
    expect(report.steps.every(step => step.outcome === 'passed')).toBe(true);
  });
});
