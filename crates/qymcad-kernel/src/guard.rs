//! THE ONE DOOR OF AN OPERATION WITH A SIZE: what OCCT is known to get wrong about sizes is handled here, once, and
//! every operation that has a size goes through it.
//!
//! OCCT fails exactly at the boundary of degeneracy. A blend whose edge lands exactly on another edge or face - a
//! chamfer of 4 on an edge 4 from a step, a fillet of a radius equal to half a 10 mm cube - is refused, while a
//! value a hair smaller builds. A person drawing to a dimension lands there on purpose, so the limiting case is
//! retried stepped back by an amount invisible in the geometry, the way mature systems build it.

use qymcad_core::geom::Built;
use qymcad_core::model::Id;

/// HOW FAR AN OPERATION OF A KIND STEPS BACK, measured per kind: the first step that builds is taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retreat {
    /// Fillets and chamfers. A fillet of R5 on a 10 mm cube took at 1 - 2e-8 (4.9999999). A chamfer needs the
    /// coarser step: a leg of 1.99999999 failed where 1.99998 worked, so 1 - 1e-5.
    Blend,
}

impl Retreat {
    /// The factors on the size after the asked one, finest first.
    fn steps(self) -> &'static [f64] {
        match self {
            Retreat::Blend => &[1.0 - 2e-8, 1.0 - 1e-5],
        }
    }
}

/// RUN `build` AT THE ASKED SIZE, and on a refusal stepped back. `build` takes the factor on its size (1.0 first) and
/// scales every length of the operation by it - both legs of a chamfer, every radius of a variable fillet - and
/// nothing else: an angle stays an angle.
///
/// The fast path costs nothing: the steps run only after a refusal.
pub fn sized<T>(retreat: Retreat, build: impl Fn(f64) -> Option<T>) -> Option<T> {
    std::iter::once(1.0).chain(retreat.steps().iter().copied()).find_map(build)
}

/// THE DOOR EVERY RESULT OF THE KERNEL GOES THROUGH before it becomes a body of the document: cleaned, checked, cached.
///
/// Every operation of `Kernel` that builds a body ends here, and nothing else puts a body into the cache: a check over
/// the source (`every_operation_goes_through_the_door`) holds both, so a new operation cannot skip what this does.
impl crate::kernel::OcctKernel {
    /// Tessellate the result and cache its shape under `body`, returning the mesh and faces of the first body.
    pub(crate) fn finish(&self, body: Id, mut shape: crate::Shape) -> Result<Built, qymcad_core::errors::CoreError> {
        // Merge every solid into one mesh: a pattern, a mirror together with its original, or a cut that fell
        // apart is a compound of several bodies, while a timeline node is one body. Taking only the first solid
        // made a pattern or a mirror show a single copy.
        //
        // The tessellation deflection adapts to the size of the body; a fixed 0.5 mm left small parts faceted
        // and huge ones unmanageable.
        // A gate: a broken body does not become a part.
        //
        // The result of every kernel operation passes through this funnel, and until now it was accepted
        // silently, sound or not. The price showed in use: pushing a face returned a body with inconsistent
        // face orientations, the timeline said nothing, and everything after it fell apart. On screen there was
        // a hole instead of a wall, the renderer showing the inside; the volume read 160767 instead of 20019,
        // the integral having lost its signs; and the next operation landed on an unnamed face of that corpse.
        //
        // That particular failure was cured in the operation itself, in the order of the boolean operands, but
        // the gate stays: any operation can return an unsound result and accepting it silently is not
        // acceptable. The check lives here rather than in each of the 41 operations — one place nothing gets
        // past.
        // A face lying in two places is not a face. A fillet or a chamfer can eat a face down to zero width:
        // the inner contour meets the outer one, the remainder lies as two patches, and both carry the same id.
        // It showed while pushing a face: clicking one end lifted the opposite one too. It is resolved here in
        // the shared funnel, since any operation can leave such a face behind.
        // A STOP ASKED ON THE WAY breaks the funnel off between its steps: on a body of 144 381 faces each of them
        // took seconds to minutes, and Cancel waited for all of them. The refusal is no failure of the node - the
        // rebuild it breaks off is cancelled whole.
        let stopped = || self.stop.load(std::sync::atomic::Ordering::Relaxed).then_some(qymcad_core::errors::CoreError::EmptyResult);
        shape.heal_pinched_faces();
        if let Some(e) = stopped() {
            return Err(e);
        }
        // a crack narrower than 1e-5 left standing (a rounding as wide as its wall) breaks the next blend beside it
        shape.drop_slivers();
        if let Some(e) = stopped() {
            return Err(e);
        }
        // Emptiness is asked about first. An empty shape fails the validity check, so a broken body was
        // reported where in fact the operation produced nothing at all. Those are different faults calling for
        // different actions: a broken result is repaired, an empty one is redone. The order of the checks is
        // the answer.
        if shape.is_empty_shape() {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        // Whether the part fell into pieces is deliberately not asked here, tempting as it was: a pattern and
        // a mirror legitimately give a compound of several bodies, and a blanket ban breaks them. The
        // `OperationSplitBody` error stays for the operations where several bodies really are a fault.
        // An unsound shape is repaired before being rejected. The kernel sometimes returns a body with a minor
        // flaw: the check rejects it and a broken body is reported where the part is essentially correct and
        // repairs with a standard tool. A red node is left only for what could not be repaired.
        if !shape.is_valid() {
            if let Some(e) = stopped() {
                return Err(e);
            }
            match shape.healed() {
                Some(fixed) => shape = fixed,
                None => return Err(qymcad_core::errors::CoreError::BrokenSolid),
            }
        }
        // Emptiness is not a part either. On the same body, pushing an inner wall gave a formally sound result
        // of zero volume: the part disappeared and the timeline said nothing. A negative volume means a shell
        // turned inside out, which is the same corpse.
        //
        // A sheet, however, has no volume by nature. Judging a surface by a zero volume would keep every
        // surface out of the document, that is, close the whole design layer at the door. So what the shape is
        // gets asked first: a solid is measured by volume, a sheet by whether it exists at all.
        if shape.is_empty_shape() {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        if !shape.is_sheet() && shape.volume() <= 1e-9 {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        if let Some(e) = stopped() {
            return Err(e);
        }
        let out = shape.tessellate_merged_auto(self.k()).ok_or(qymcad_core::errors::CoreError::EmptyResult)?;
        self.shapes.borrow_mut().insert(body, shape);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A build refusing exactly at its size and taking anything below it: the step back builds it.
    #[test]
    fn the_limiting_case_is_built_stepped_back() {
        let asked = 4.0;
        let got = sized(Retreat::Blend, |f| (asked * f < asked).then_some(asked * f));
        assert!(got.is_some_and(|v| asked - v < 1e-6), "the limiting case is not built a hair short of the size: {got:?}");
    }

    /// A build that takes at once is asked once: the fast path does not pay for the steps.
    #[test]
    fn a_build_that_takes_is_asked_once() {
        let asked = std::cell::Cell::new(0);
        let got = sized(Retreat::Blend, |f| {
            asked.set(asked.get() + 1);
            Some(f)
        });
        assert_eq!((got, asked.get()), (Some(1.0), 1), "the fast path ran the steps back");
    }
}
