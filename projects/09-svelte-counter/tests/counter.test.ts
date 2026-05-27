import { describe, expect, it } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';

import Counter from '../src/Counter.svelte';

describe('Counter component (Phase 9 — runes)', () => {
  it('renders the initial value from the bound prop', () => {
    const { getByTestId } = render(Counter, { props: { value: 7 } });
    expect(getByTestId('value').textContent).toBe('7');
  });

  it('increments when + is clicked', async () => {
    const { getByTestId } = render(Counter, { props: { value: 0 } });
    await fireEvent.click(getByTestId('inc'));
    expect(getByTestId('value').textContent).toBe('1');
  });

  it('decrements when − is clicked', async () => {
    const { getByTestId } = render(Counter, { props: { value: 5 } });
    await fireEvent.click(getByTestId('dec'));
    expect(getByTestId('value').textContent).toBe('4');
  });

  it('honors the `min` floor — won’t decrement past it', async () => {
    const { getByTestId } = render(Counter, { props: { value: 0, min: 0 } });
    await fireEvent.click(getByTestId('dec'));
    expect(getByTestId('value').textContent).toBe(
      '0',
      // If this fails, the `if (value - step >= min)` guard in
      // Counter.svelte regressed — a real bug, not flakiness.
    );
  });

  it('resets to 0', async () => {
    const { getByTestId } = render(Counter, { props: { value: 42 } });
    await fireEvent.click(getByTestId('reset'));
    expect(getByTestId('value').textContent).toBe('0');
  });

  it('respects a non-default step', async () => {
    const { getByTestId } = render(Counter, { props: { value: 0, step: 5 } });
    await fireEvent.click(getByTestId('inc'));
    await fireEvent.click(getByTestId('inc'));
    expect(getByTestId('value').textContent).toBe('10');
  });
});
